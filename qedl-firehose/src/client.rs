use crate::command::FirehoseCommand;
use crate::error::{FirehoseError, Result};
use crate::response::FirehoseResponse;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use qedl_core::{Event, EventSink, FirehoseEvent, emit_event};
use qedl_transport::Transport;
use std::sync::Arc;
use std::time::Duration;

const READ_BUF_SIZE: usize = 64 * 1024;

pub struct FirehoseClient {
    pub(crate) memory_name: String,
    pub(crate) sector_size: u32,
    pub(crate) max_payload_size: u32,
    pub(crate) max_payload_size_from_target: Option<u32>,
    pub(crate) max_payload_size_to_target_supported: Option<u32>,
    pub(crate) max_xml_size: Option<u32>,
    pub(crate) target_name: String,
    pub(crate) version: Option<String>,
    pub(crate) total_sectors: u64,
    initialized: bool,
    /// Buffer for leftover bytes from read_response that belong to raw data, not XML
    leftover: BytesMut,
    /// Reusable I/O buffer to avoid per-command allocation
    read_buf: Vec<u8>,
    event_sink: Option<Arc<dyn EventSink>>,
}

impl FirehoseClient {
    pub fn new() -> Self {
        Self {
            memory_name: "eMMC".to_string(),
            sector_size: 512,
            max_payload_size: 1024 * 1024,
            max_payload_size_from_target: None,
            max_payload_size_to_target_supported: None,
            max_xml_size: None,
            target_name: "unknown".to_string(),
            version: None,
            total_sectors: 0,
            initialized: false,
            leftover: BytesMut::new(),
            read_buf: vec![0u8; READ_BUF_SIZE],
            event_sink: None,
        }
    }

    pub fn with_event_sink(mut self, sink: Option<Arc<dyn EventSink>>) -> Self {
        self.event_sink = sink;
        self
    }

    fn emit(&self, event: FirehoseEvent) {
        emit_event(&self.event_sink, Event::Firehose(event));
    }

    pub async fn configure(&mut self, transport: &mut dyn Transport) -> Result<()> {
        self.emit(FirehoseEvent::ConfigureStarted);
        let cmd = FirehoseCommand::Configure {
            memory_name: self.memory_name.clone(),
            target_name: self.target_name.clone(),
            skip_storage_init: false,
            zlp_aware_host: true,
            max_payload_size: self.max_payload_size,
        };

        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            return Err(FirehoseError::ConfigureFailed {
                reason: resp.error.unwrap_or_else(|| "unknown error".to_string()),
            });
        }

        if let Some(name) = &resp.config.memory_name {
            self.memory_name = name.clone();
        }
        if let Some(ss) = resp.config.sector_size {
            self.sector_size = ss;
        }
        if let Some(mps) = resp.config.max_payload_size {
            self.max_payload_size = mps;
        }
        if let Some(mpsft) = resp.config.max_payload_size_from_target {
            self.max_payload_size_from_target = Some(mpsft);
        }
        if let Some(mpstts) = resp.config.max_payload_size_to_target_supported {
            self.max_payload_size_to_target_supported = Some(mpstts);
        }
        if let Some(mxs) = resp.config.max_xml_size {
            self.max_xml_size = Some(mxs);
        }
        if let Some(ts) = resp.config.target_name {
            self.target_name = ts;
        }
        if let Some(v) = resp.config.version {
            self.version = Some(v);
        }
        if let Some(ts) = resp.config.total_sectors {
            self.total_sectors = ts;
        }

        self.initialized = true;
        self.emit(FirehoseEvent::ConfigureComplete);
        tracing::info!(
            "Firehose Configured: TargetName={}, Memory={}, SectorSize={}, MaxPayload={}, MaxPayloadFromTarget={}, MaxPayloadSupported={}, MaxXML={}, Version={}",
            self.target_name,
            self.memory_name,
            self.sector_size,
            self.max_payload_size,
            self.max_payload_size_from_target
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.max_payload_size_to_target_supported
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.max_xml_size.map_or("N/A".to_string(), |v| v.to_string()),
            self.version.as_deref().unwrap_or("N/A"),
        );
        Ok(())
    }

    pub async fn execute_command(
        &mut self,
        transport: &mut dyn Transport,
        command: &FirehoseCommand,
    ) -> Result<FirehoseResponse> {
        let cmd_name = command.name();
        let inner = command.to_xml();
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8" ?><data>{}</data>"#, inner);

        // Semantic trace: only command name (TRACE level)
        tracing::trace!("Firehose -> {}", cmd_name);

        // Full XML dump only with trace-xml feature (TRACE level)
        #[cfg(feature = "trace-xml")]
        tracing::trace!(xml = %xml, "Firehose XML sent");

        let _ = transport.flush().await;
        transport.write(xml.as_bytes()).await?;

        let response_xml = self.read_response(transport).await?;

        // Full XML dump only with trace-xml feature (TRACE level)
        #[cfg(feature = "trace-xml")]
        tracing::trace!(xml = %response_xml, "Firehose XML received");

        let resp =
            FirehoseResponse::from_xml(&response_xml).map_err(|e| FirehoseError::InvalidResponse { reason: e })?;

        // Log semantic trace: ACK/NAK
        tracing::trace!("Firehose <- {}", if resp.is_ack() { "ACK" } else { "NAK" });

        // Log device logs at trace level
        for log_msg in &resp.logs {
            tracing::trace!("Firehose log: {}", log_msg);
        }

        Ok(resp)
    }

    /// Send raw XML string to the device without waiting for response.
    async fn send_xml(&mut self, transport: &mut dyn Transport, xml: &str) -> Result<()> {
        let _ = transport.flush().await;
        transport.write(xml.as_bytes()).await?;
        Ok(())
    }

    /// Read XML response, preserving any extra bytes (raw data) in self.leftover.
    /// This prevents raw data that arrives in the same serial read as XML from being lost.
    /// It continues reading until a <response ... /> tag is found.
    async fn read_response(&mut self, transport: &mut dyn Transport) -> Result<String> {
        let buf = &mut self.read_buf;
        let mut response = BytesMut::with_capacity(8192); // Pre-allocate response buffer

        // Start with any leftover from previous read_response
        if !self.leftover.is_empty() {
            response.extend_from_slice(&self.leftover);
            self.leftover.clear();
        }

        let mut empty_reads = 0u32;
        let max_empty_reads = 10;

        loop {
            transport.set_timeout(Duration::from_millis(1000));

            let n = match transport.read(buf).await {
                Ok(0) => 0,
                Ok(n) => n,
                Err(_) => {
                    tracing::trace!("Firehose read_response: transport timeout, retrying...");
                    0
                }
            };
            if n == 0 {
                // If we already have some data, check if it's a complete response
                // before giving up on empty read
                if !response.is_empty() {
                    let text = String::from_utf8_lossy(&response);
                    if text.contains("<response ") && (text.contains("/>") || text.contains("</response>")) {
                        tracing::trace!("Firehose response found in existing buffer after timeout");
                        // We found it, proceed to extract and return
                    } else {
                        empty_reads += 1;
                        if empty_reads > max_empty_reads {
                            tracing::warn!(
                                "Firehose read_response: {} consecutive empty reads, giving up. Current buffer: {}",
                                empty_reads,
                                text
                            );
                            return Err(FirehoseError::Timeout {
                                command: "read_response".to_string(),
                            });
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                } else {
                    empty_reads += 1;
                    if empty_reads > max_empty_reads {
                        return Err(FirehoseError::Timeout {
                            command: "read_response".to_string(),
                        });
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            }

            empty_reads = 0;
            if n > 0 {
                response.extend_from_slice(&buf[..n]);
            }

            let text = String::from_utf8_lossy(&response);

            // We are looking for the <response ... /> tag.
            // Some devices wrap everything in <data>...</data>, some don't.
            // The most reliable way is to find the <response tag and then its closure.
            if let Some(resp_start) = text.find("<response ") {
                // Find where this response ends. It could be "/>" or "</response>"
                let mut resp_end = text[resp_start..].find("/>").map(|i| i + 2);
                if resp_end.is_none() {
                    resp_end = text[resp_start..].find("</response>").map(|i| i + "</response>".len());
                }

                if let Some(rel_end) = resp_end {
                    let abs_end = resp_start + rel_end;

                    // If there's a </data> after the response, include it too
                    let mut final_end = abs_end;
                    if let Some(data_end) = text[abs_end..].find("</data>") {
                        final_end = abs_end + data_end + "</data>".len();
                    }

                    if final_end < response.len() {
                        self.leftover = response.split_off(final_end);
                    }

                    let result = String::from_utf8_lossy(&response).to_string();
                    tracing::trace!(
                        "Firehose response complete ({} bytes, leftover: {})",
                        response.len(),
                        self.leftover.len()
                    );
                    return Ok(result);
                }
            }

            if response.len() > 128 * 1024 {
                tracing::warn!("Firehose response too large: {} bytes", response.len());
                return Err(FirehoseError::InvalidResponse {
                    reason: "response too large".to_string(),
                });
            }
        }
    }

    pub async fn read_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<Bytes> {
        let total_bytes = (num_sectors * self.sector_size as u64) as usize;
        let start_time = std::time::Instant::now();
        tracing::trace!(
            "Firehose read: LUN={}, Sector={}, Count={}, Bytes={}",
            physical_partition,
            start_sector,
            num_sectors,
            total_bytes
        );

        self.emit(FirehoseEvent::ReadStarted {
            lun: physical_partition,
            start: start_sector,
            count: num_sectors,
        });

        let cmd = FirehoseCommand::Read {
            sector_size: self.sector_size,
            num_sectors,
            physical_partition,
            start_sector,
        };

        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            tracing::error!(error = resp.error.as_deref().unwrap_or("unknown"), "Firehose read NAK");
            return Err(FirehoseError::Nak {
                command: "read".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }

        let mut data = vec![0u8; total_bytes];
        let mut read = 0;

        // Special case for MSM8937 and similar: data might be in the logs as hex strings
        // if rawmode=true is "fake".
        let mut data_from_logs = Vec::new();
        for log in &resp.logs {
            if log.contains("0x") {
                let parts: Vec<&str> = log.split_whitespace().collect();
                for part in parts {
                    if let Some(hex_val) = part.strip_prefix("0x")
                        && let Ok(val) = u8::from_str_radix(hex_val, 16)
                    {
                        data_from_logs.push(val);
                    }
                }
            }
        }

        if !data_from_logs.is_empty() {
            tracing::trace!(
                count = data_from_logs.len(),
                "Firehose read: extracted bytes from logs (pseudo-rawmode)"
            );
            let n = std::cmp::min(data_from_logs.len(), data.len());
            data[..n].copy_from_slice(&data_from_logs[..n]);
            read = n;
        }

        // Use any leftover data from read_response first (XML+RAW in same serial read)
        if read < total_bytes && !self.leftover.is_empty() {
            let n = std::cmp::min(self.leftover.len(), data.len() - read);
            let consumed = self.leftover.split_to(n);
            data[read..read + n].copy_from_slice(&consumed);
            read += n;
            tracing::trace!(count = n, "Firehose read: consumed bytes from leftover");
        }

        while read < total_bytes {
            let n = transport.read(&mut data[read..]).await?;
            if n == 0 {
                tracing::warn!(read = read, total = total_bytes, "Firehose read data: EOF");
                return Err(FirehoseError::Timeout {
                    command: "read".to_string(),
                });
            }
            read += n;

            // Emit progress event
            self.emit(FirehoseEvent::ReadProgress {
                current: read as u64,
                total: total_bytes as u64,
            });
        }

        // After reading the raw data, we MUST read the final completion response (usually <response value="ACK" />)
        // If we don't, it will stay in the transport buffer and corrupt the next command.
        tracing::trace!("Firehose read: raw data complete, waiting for completion response...");
        let final_resp_xml = self.read_response(transport).await?;
        let final_resp =
            FirehoseResponse::from_xml(&final_resp_xml).map_err(|e| FirehoseError::InvalidResponse { reason: e })?;

        if !final_resp.is_ack() {
            tracing::error!(
                error = ?final_resp.error,
                "Firehose read: completion response was NAK"
            );
            return Err(FirehoseError::Nak {
                command: "read".to_string(),
                reason: final_resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        } else {
            tracing::trace!("Firehose read: completion response ACK received");
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        self.emit(FirehoseEvent::ReadComplete);
        tracing::trace!(
            "Firehose read complete: {} bytes in {:.3}s ({:.2} MiB/s)",
            read,
            elapsed,
            (read as f64 / 1024.0 / 1024.0) / elapsed
        );
        if data.len() >= 8 && &data[0..8] == b"EFI PART" {
            tracing::debug!("GPT signature found at start of data");
        }

        Ok(Bytes::from(data))
    }

    pub async fn program_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        tracing::trace!(
            "Firehose program: LUN={}, Sector={}, Count={}, Bytes={}",
            physical_partition,
            start_sector,
            num_sectors,
            data.len()
        );

        let cmd = FirehoseCommand::Program {
            sector_size: self.sector_size,
            num_sectors,
            physical_partition,
            start_sector,
            filename: None,
        };

        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            tracing::error!(
                error = resp.error.as_deref().unwrap_or("unknown"),
                "Firehose program NAK"
            );
            return Err(FirehoseError::Nak {
                command: "program".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }

        transport.write(data).await?;

        let final_resp_xml = self.read_response(transport).await?;
        let final_resp =
            FirehoseResponse::from_xml(&final_resp_xml).map_err(|e| FirehoseError::InvalidResponse { reason: e })?;
        if !final_resp.is_ack() {
            tracing::error!(
                error = ?final_resp.error,
                "Firehose program: completion response was NAK"
            );
            return Err(FirehoseError::Nak {
                command: "program".to_string(),
                reason: final_resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        tracing::trace!(
            "Firehose program complete: {} bytes in {:.3}s ({:.2} MiB/s)",
            data.len(),
            elapsed,
            (data.len() as f64 / 1024.0 / 1024.0) / elapsed
        );

        Ok(())
    }

    pub async fn erase_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<()> {
        tracing::debug!(
            "Firehose erase: LUN={}, sector={}, count={}",
            physical_partition,
            start_sector,
            num_sectors
        );

        let cmd = FirehoseCommand::Erase {
            sector_size: self.sector_size,
            num_sectors,
            physical_partition,
            start_sector,
        };

        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            tracing::error!("Firehose erase NAK: {}", resp.error.as_deref().unwrap_or("unknown"));
            return Err(FirehoseError::Nak {
                command: "erase".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }

        tracing::debug!("Firehose erase complete");
        Ok(())
    }

    pub async fn get_storage_info(&mut self, transport: &mut dyn Transport) -> Result<FirehoseResponse> {
        self.execute_command(transport, &FirehoseCommand::GetStorageInfo).await
    }

    /// Get SHA256 digest of partition sectors from device.
    /// Returns the hex-encoded SHA256 hash string.
    pub async fn get_sha256_digest(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<String> {
        let cmd = FirehoseCommand::GetSha256Digest {
            sector_size: self.sector_size,
            num_sectors,
            physical_partition,
            start_sector,
        };
        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            return Err(FirehoseError::Nak {
                command: "getsha256digest".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }
        // The digest is returned in the log messages
        // Format: "Digest: <hex_string>" or just the hex string
        for log in &resp.logs {
            if let Some(idx) = log.find("Digest ") {
                return Ok(log[idx + 7..].to_string());
            }
            // Some implementations just return the hash directly
            if log.len() == 64 && log.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(log.clone());
            }
        }
        // If no digest found in logs, return the last log entry (some devices return it that way)
        resp.logs.last().cloned().ok_or_else(|| FirehoseError::InvalidResponse {
            reason: "No digest in response".to_string(),
        })
    }

    /// Read memory at physical address. Returns raw bytes.
    pub async fn peek(&mut self, transport: &mut dyn Transport, address: u64, size: u32) -> Result<Vec<u8>> {
        let cmd = FirehoseCommand::Peek { address, size };
        let inner = cmd.to_xml();
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8" ?><data>{}</data>"#, inner);
        tracing::trace!("Firehose -> {}", cmd.name());
        #[cfg(feature = "trace-xml")]
        tracing::trace!(xml = %xml, "Firehose XML sent");
        self.send_xml(transport, &xml).await?;

        // Peek responses come as hex-encoded log entries, not a normal ACK/NAK
        // We need to accumulate all log entries until we get a response
        let mut data = Vec::new();
        let buf = &mut self.read_buf;
        let mut total_read = 0usize;

        loop {
            match transport.read(buf).await {
                Ok(0) => {
                    tracing::trace!("peek: EOF after {} bytes", total_read);
                    break;
                }
                Ok(n) => {
                    total_read += n;
                    let text = String::from_utf8_lossy(&buf[..n]);

                    // Check for error responses
                    if text.contains("NAK") || text.contains("Invalid parameters") || text.contains("can't") {
                        tracing::error!("peek NAK at addr=0x{:X}: {}", address, text.trim());
                        return Err(FirehoseError::Nak {
                            command: "peek".to_string(),
                            reason: text.trim().to_string(),
                        });
                    }

                    // Parse hex values from log entries
                    // Format: "0x22 0x00 0x00 0xEA 0x70 0x00 0x00 0xEA"
                    for word in text.split_whitespace() {
                        if (word.starts_with("0x") || word.starts_with("0X"))
                            && let Ok(byte) = u8::from_str_radix(&word[2..], 16)
                        {
                            data.push(byte);
                        }
                    }

                    // Check if we got the complete response
                    if text.contains("</data>") || text.contains("<response") {
                        tracing::debug!(
                            addr = format!("0x{:X}", address),
                            bytes = data.len(),
                            "Firehose <- peek"
                        );
                        break;
                    }
                }
                Err(e) => {
                    if data.is_empty() {
                        return Err(e.into());
                    }
                    // Timeout with partial data is OK for peek
                    tracing::trace!("peek: read timeout with {} bytes", data.len());
                    break;
                }
            }
        }

        Ok(data)
    }

    /// Write memory at physical address.
    pub async fn poke(&mut self, transport: &mut dyn Transport, address: u64, data: &[u8]) -> Result<()> {
        let cmd = FirehoseCommand::Poke {
            address,
            data: data.to_vec(),
        };
        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            return Err(FirehoseError::Nak {
                command: "poke".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }
        Ok(())
    }

    pub async fn reboot(&mut self, transport: &mut dyn Transport) -> Result<()> {
        let cmd = FirehoseCommand::Power {
            value: "reset".to_string(),
        };
        let resp = self.execute_command(transport, &cmd).await?;
        if !resp.is_ack() {
            return Err(FirehoseError::Nak {
                command: "reboot".to_string(),
                reason: resp.error.unwrap_or_else(|| "unknown".to_string()),
            });
        }
        Ok(())
    }

    pub async fn raw_xml(&mut self, transport: &mut dyn Transport, xml: &str) -> Result<FirehoseResponse> {
        self.execute_command(transport, &FirehoseCommand::RawXml(xml.to_string()))
            .await
    }

    /// Drain the device's initialization messages after Sahara handshake.
    /// Waits up to 3s for the Firehose loader to boot, then drains any init messages.
    pub async fn drain_initial_messages(&mut self, transport: &mut dyn Transport) -> Result<()> {
        tracing::debug!("Waiting for Firehose mode...");
        transport.set_timeout(Duration::from_millis(500));

        let buf = &mut self.read_buf;
        let mut got_data = false;
        let mut total = 0usize;

        for attempt in 0..6 {
            match transport.read(buf).await {
                Ok(0) | Err(_) => {
                    tracing::trace!("Waiting for loader boot (attempt {})", attempt + 1);
                    continue;
                }
                Ok(n) => {
                    got_data = true;
                    total += n;
                    let text = String::from_utf8_lossy(&buf[..n]);
                    // Check if this chunk contains a valid response
                    if text.contains("<response ") || text.contains("</data>") {
                        tracing::debug!("Drain: response ready ({} bytes)", total);
                        return Ok(());
                    }
                    // Keep draining
                    tracing::trace!("Drain: {} bytes (waiting for response)", n);
                    break;
                }
            }
        }

        if !got_data {
            tracing::debug!("No init data received after Sahara");
            return Ok(());
        }

        // Drain remaining data until we find a response or timeout
        loop {
            match transport.read(buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    total += n;
                    let text = String::from_utf8_lossy(&buf[..n]);
                    if text.contains("<response ") || text.contains("</data>") {
                        tracing::debug!("Drain: response ready ({} bytes)", total);
                        return Ok(());
                    }
                }
            }
        }
        tracing::debug!("Drain complete ({} bytes, no response found)", total);
        Ok(())
    }

    pub fn sector_size(&self) -> u32 {
        self.sector_size
    }

    pub fn max_payload_size(&self) -> u32 {
        self.max_payload_size
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn memory_name(&self) -> &str {
        &self.memory_name
    }

    pub fn target_name(&self) -> &str {
        &self.target_name
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn max_payload_size_from_target(&self) -> Option<u32> {
        self.max_payload_size_from_target
    }

    pub fn max_payload_size_to_target_supported(&self) -> Option<u32> {
        self.max_payload_size_to_target_supported
    }

    pub fn max_xml_size(&self) -> Option<u32> {
        self.max_xml_size
    }

    pub fn total_sectors(&self) -> u64 {
        self.total_sectors
    }

    /// Update sector_size and total_sectors from a getstorageinfo response.
    pub fn update_from_storage_info(&mut self, sector_size: Option<u32>, total_sectors: Option<u64>) {
        if let Some(ss) = sector_size {
            self.sector_size = ss;
        }
        if let Some(ts) = total_sectors {
            self.total_sectors = ts;
        }
    }

    pub fn set_memory_name(&mut self, name: String) {
        self.memory_name = name;
    }
}

impl Default for FirehoseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::FirehoseProtocol for FirehoseClient {
    async fn configure(&mut self, transport: &mut dyn Transport) -> Result<()> {
        self.configure(transport).await
    }
    async fn execute_command(
        &mut self,
        transport: &mut dyn Transport,
        command: &crate::command::FirehoseCommand,
    ) -> Result<FirehoseResponse> {
        self.execute_command(transport, command).await
    }
    async fn read_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<Bytes> {
        self.read_sectors(transport, physical_partition, start_sector, num_sectors)
            .await
    }
    async fn program_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> Result<()> {
        self.program_sectors(transport, physical_partition, start_sector, num_sectors, data)
            .await
    }
    async fn erase_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<()> {
        self.erase_sectors(transport, physical_partition, start_sector, num_sectors)
            .await
    }
    async fn get_storage_info(&mut self, transport: &mut dyn Transport) -> Result<FirehoseResponse> {
        self.get_storage_info(transport).await
    }
    async fn get_sha256_digest(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<String> {
        self.get_sha256_digest(transport, physical_partition, start_sector, num_sectors)
            .await
    }
    async fn peek(&mut self, transport: &mut dyn Transport, address: u64, size: u32) -> Result<Vec<u8>> {
        self.peek(transport, address, size).await
    }
    async fn poke(&mut self, transport: &mut dyn Transport, address: u64, data: &[u8]) -> Result<()> {
        self.poke(transport, address, data).await
    }
    async fn reboot(&mut self, transport: &mut dyn Transport) -> Result<()> {
        self.reboot(transport).await
    }
    async fn raw_xml(&mut self, transport: &mut dyn Transport, xml: &str) -> Result<FirehoseResponse> {
        self.raw_xml(transport, xml).await
    }
    async fn drain_initial_messages(&mut self, transport: &mut dyn Transport) -> Result<()> {
        self.drain_initial_messages(transport).await
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }
    fn max_payload_size(&self) -> u32 {
        self.max_payload_size
    }
    fn is_initialized(&self) -> bool {
        self.initialized
    }
    fn memory_name(&self) -> &str {
        &self.memory_name
    }
    fn target_name(&self) -> &str {
        &self.target_name
    }
    fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    fn max_payload_size_from_target(&self) -> Option<u32> {
        self.max_payload_size_from_target
    }
    fn max_payload_size_to_target_supported(&self) -> Option<u32> {
        self.max_payload_size_to_target_supported
    }
    fn max_xml_size(&self) -> Option<u32> {
        self.max_xml_size
    }
    fn total_sectors(&self) -> u64 {
        self.total_sectors
    }
    fn update_from_storage_info(&mut self, sector_size: Option<u32>, total_sectors: Option<u64>) {
        if let Some(ss) = sector_size {
            self.sector_size = ss;
        }
        if let Some(ts) = total_sectors {
            self.total_sectors = ts;
        }
    }
    fn set_memory_name(&mut self, name: String) {
        self.memory_name = name;
    }
}
