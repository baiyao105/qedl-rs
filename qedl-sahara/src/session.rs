//! Sahara 握手协议

use crate::error::{Result, SaharaError};
use crate::protocol::*;
use bytes::Bytes;
use qedl_core::protocol::sahara::exec_cmd;
use qedl_core::{Event, EventSink, SaharaEvent, emit_event};
use qedl_transport::Transport;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaharaState {
    WaitingHello,
    HelloReceived,
    WaitingDataRequest,
    Transferring,
    Done,
    Error,
}

/// Device information obtained from Sahara exec commands.
#[derive(Debug, Clone, Default)]
pub struct SaharaDeviceInfo {
    /// Raw MSM hardware ID bytes (8 bytes)
    pub msm_hw_id: Option<Vec<u8>>,
    /// Chip serial number
    pub serial_num: Option<u64>,
}

pub struct SaharaSession<T: Transport> {
    state: SaharaState,
    protocol_version: u32,
    max_chunk_size: usize,
    transport: T,
    event_sink: Option<Arc<dyn EventSink>>,
}

impl<T: Transport> SaharaSession<T> {
    pub fn new(transport: T) -> Self {
        Self {
            state: SaharaState::WaitingHello,
            protocol_version: consts::PROTOCOL_VERSION,
            max_chunk_size: consts::DEFAULT_CHUNK_SIZE,
            transport,
            event_sink: None,
        }
    }

    pub fn with_event_sink(mut self, sink: Option<Arc<dyn EventSink>>) -> Self {
        self.event_sink = sink;
        self
    }

    fn emit(&self, event: SaharaEvent) {
        emit_event(&self.event_sink, Event::Sahara(event));
    }

    /// Execute Sahara handshake.
    ///
    /// 1. Read Hello (1s timeout)
    /// 2. If no Hello → PblHack recovery
    /// 3. Send HelloResponse
    /// 4. Optionally query device info (MSM HW ID, serial, etc.)
    /// 5. Loop ReadData requests, upload loader
    /// 6. Send Done, device enters Firehose mode
    ///
    /// Consumes self and returns (transport, device_info) on success.
    pub async fn handshake(
        mut self,
        loader_path: Option<&Path>,
        mode: SaharaMode,
    ) -> std::result::Result<(T, SaharaDeviceInfo), SaharaError> {
        tracing::info!("Sahara handshake started");
        self.emit(SaharaEvent::HandshakeStarted);
        let start = std::time::Instant::now();

        let hello = match self.read_hello().await {
            Ok(h) => h,
            Err(SaharaError::AlreadyInFirehose) => {
                tracing::info!("Device already in Firehose mode, skipping Sahara");
                self.emit(SaharaEvent::AlreadyInFirehoseMode);
                return Ok((self.transport, SaharaDeviceInfo::default()));
            }
            Err(SaharaError::HelloFailed) => {
                // Hello not received, try NOP to check if device is in Firehose mode
                tracing::debug!("Hello not received, sending NOP to check Firehose mode");
                if self.try_firehose_nop().await {
                    tracing::info!("Device is in Firehose mode (NOP ACK received)");
                    self.emit(SaharaEvent::AlreadyInFirehoseMode);
                    return Ok((self.transport, SaharaDeviceInfo::default()));
                }
                // No loader specified — cannot proceed with Sahara
                if loader_path.is_none() {
                    tracing::info!("No loader specified and device not in Firehose mode");
                    return Err(SaharaError::NotInFirehose);
                }
                tracing::debug!("NOP failed, trying PblHack recovery");
                self.pbl_hack(mode).await?;
                match self.read_hello().await {
                    Ok(h) => h,
                    Err(SaharaError::AlreadyInFirehose) => {
                        tracing::info!("Device already in Firehose mode after PblHack, skipping Sahara");
                        self.emit(SaharaEvent::AlreadyInFirehoseMode);
                        return Ok((self.transport, SaharaDeviceInfo::default()));
                    }
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        };

        tracing::debug!(
            "Sahara Hello: version={}, min_version={}, mode={:?}",
            hello.version,
            hello.version_min,
            hello.mode
        );

        self.send_hello_response(&hello, mode).await?;

        // Query device info if device supports command mode
        let mut device_info = SaharaDeviceInfo::default();
        if hello.mode == SaharaMode::Command {
            tracing::debug!("Device in command mode, querying info");
            match self.get_msm_hw_id().await {
                Ok(id) => {
                    tracing::debug!("MSM HW ID: {:02X?}", id);
                    device_info.msm_hw_id = Some(id);
                }
                Err(e) => tracing::trace!("get_msm_hw_id failed: {}", e),
            }
            match self.get_serial_num().await {
                Ok(num) => {
                    tracing::debug!("Serial: 0x{:016X}", num);
                    device_info.serial_num = Some(num);
                }
                Err(e) => tracing::trace!("get_serial_num failed: {}", e),
            }
        }

        // loader_path is guaranteed Some here (None returns early above)
        let loader_path = loader_path.unwrap();

        tracing::debug!(path = ?loader_path, "Loading Sahara loader");
        let loader_data = Bytes::from(std::fs::read(loader_path).map_err(|e| SaharaError::TransferFailed {
            offset: 0,
            reason: format!("failed to read loader file: {}", e),
        })?);
        tracing::debug!(
            size = loader_data.len(),
            "Loader loaded ({} KB)",
            loader_data.len() / 1024
        );

        self.upload_loader(&loader_data).await?;

        tracing::debug!("Sending Sahara Done");
        self.send_done().await?;

        self.state = SaharaState::Done;
        self.emit(SaharaEvent::HandshakeComplete);
        tracing::info!("Sahara handshake complete ({:.1}s)", start.elapsed().as_secs_f64());
        Ok((self.transport, device_info))
    }

    async fn read_hello(&mut self) -> Result<SaharaHello> {
        self.state = SaharaState::WaitingHello;
        let _ = self.transport.flush().await;

        self.transport.set_timeout(Duration::from_secs(1));

        tracing::trace!("Sahara reading Hello (1s timeout)");
        let mut buf = [0u8; consts::HELLO_PACKET_SIZE];
        match self.transport.read_exact(&mut buf).await {
            Ok(()) => {
                if buf[0] >= 0x20 {
                    tracing::debug!("Sahara first byte 0x{:02X} is ASCII, device in Firehose mode", buf[0]);
                    return Err(SaharaError::AlreadyInFirehose);
                }
                let cmd = u32::from_le_bytes(buf[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to parse command: {}", e),
                })?);
                if cmd != SaharaCommand::Hello as u32 {
                    tracing::debug!("Sahara expected Hello(0x01), got 0x{:08X}", cmd);
                    return Err(SaharaError::UnexpectedCommand { cmd });
                }
                tracing::trace!("Sahara Hello received (48 bytes)");
                self.state = SaharaState::HelloReceived;
                self.emit(SaharaEvent::HelloReceived);
                self.max_chunk_size =
                    u32::from_le_bytes(buf[16..20].try_into().map_err(|e| SaharaError::TransferFailed {
                        offset: 0,
                        reason: format!("failed to parse max_chunk_size: {}", e),
                    })?) as usize;
                Ok(Self::parse_hello(&buf)?)
            }
            Err(e) => {
                tracing::trace!("Sahara Hello read failed: {}", e);
                Err(SaharaError::HelloFailed)
            }
        }
    }

    /// PblHack recovery — device didn't send Hello.
    /// 1. Send HelloResponse directly (assume Hello was sent)
    /// 2. Wait for CMD_READY
    /// 3. ModeSwitch to recover
    async fn pbl_hack(&mut self, mode: SaharaMode) -> Result<()> {
        tracing::debug!("Sahara PblHack: sending HelloResponse without Hello");

        let _ = self.transport.flush().await;

        let hello_resp = SaharaHelloResponse {
            command: SaharaCommand::HelloResponse,
            packet_length: consts::HELLO_PACKET_SIZE as u32,
            version: self.protocol_version,
            version_min: consts::PROTOCOL_VERSION_MIN,
            max_command_length: 0,
            mode,
            reserved: [0; 6],
        };
        self.send_hello_response_raw(&hello_resp).await?;
        tracing::trace!("Sahara PblHack: HelloResponse sent");

        self.transport.set_timeout(Duration::from_millis(10));
        let mut rsp_buf = [0u8; 16];
        match self.transport.read_exact(&mut rsp_buf).await {
            Ok(()) => {
                let cmd = u32::from_le_bytes(rsp_buf[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to parse command: {}", e),
                })?);
                if cmd == SaharaCommand::CmdReady as u32 {
                    tracing::trace!("Sahara PblHack: CMD_READY");
                } else if cmd == SaharaCommand::EndTransfer as u32 {
                    let status =
                        u32::from_le_bytes(rsp_buf[8..12].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse status: {}", e),
                        })?);
                    if status == SaharaStatus::InvalidCmd as u32 {
                        tracing::trace!("Sahara PblHack: NAK(INVALID_CMD)");
                    }
                }
            }
            Err(_) => {
                tracing::trace!("Sahara PblHack: no response, sending ModeSwitch");
            }
        }

        self.mode_switch(mode).await?;
        tracing::debug!("Sahara PblHack recovery complete");
        Ok(())
    }

    /// Try to detect if device is in Firehose mode by sending a NOP command.
    /// Returns true if device responds with ACK.
    async fn try_firehose_nop(&mut self) -> bool {
        let nop_xml = b"<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<data>\n<nop />\n</data>";

        let _ = self.transport.flush().await;
        self.transport.set_timeout(Duration::from_millis(500));

        if self.transport.write(nop_xml).await.is_err() {
            tracing::trace!("NOP: write failed");
            return false;
        }

        let mut buf = [0u8; 512];
        match self.transport.read(&mut buf).await {
            Ok(n) if n > 0 => {
                let response = String::from_utf8_lossy(&buf[..n]);
                tracing::trace!("NOP response: {}", response.trim());
                response.contains("ACK")
            }
            _ => {
                tracing::trace!("NOP: no response");
                false
            }
        }
    }

    async fn mode_switch(&mut self, mode: SaharaMode) -> Result<()> {
        let mut pkt = [0u8; 12];
        pkt[0..4].copy_from_slice(&(SaharaCommand::SwitchMode as u32).to_le_bytes());
        pkt[4..8].copy_from_slice(&12u32.to_le_bytes());
        pkt[8..12].copy_from_slice(&(mode as u32).to_le_bytes());
        self.transport.write(&pkt).await?;
        tracing::trace!("Sahara ModeSwitch sent (mode={:?})", mode);
        Ok(())
    }

    async fn send_hello_response(&mut self, hello: &SaharaHello, mode: SaharaMode) -> Result<()> {
        let resp = SaharaHelloResponse {
            command: SaharaCommand::HelloResponse,
            packet_length: consts::HELLO_PACKET_SIZE as u32,
            version: self.protocol_version,
            version_min: consts::PROTOCOL_VERSION_MIN,
            max_command_length: 0,
            mode,
            reserved: hello.reserved,
        };
        self.send_hello_response_raw(&resp).await
    }

    async fn send_hello_response_raw(&mut self, resp: &SaharaHelloResponse) -> Result<()> {
        let mut pkt = [0u8; consts::HELLO_PACKET_SIZE];
        pkt[0..4].copy_from_slice(&(resp.command as u32).to_le_bytes());
        pkt[4..8].copy_from_slice(&resp.packet_length.to_le_bytes());
        pkt[8..12].copy_from_slice(&resp.version.to_le_bytes());
        pkt[12..16].copy_from_slice(&resp.version_min.to_le_bytes());
        pkt[16..20].copy_from_slice(&resp.max_command_length.to_le_bytes());
        pkt[20..24].copy_from_slice(&(resp.mode as u32).to_le_bytes());
        for (i, &val) in resp.reserved.iter().enumerate() {
            pkt[24 + i * 4..28 + i * 4].copy_from_slice(&val.to_le_bytes());
        }

        self.transport.write(&pkt).await?;
        tracing::trace!(
            "Sahara HelloResponse sent (version={}, mode={:?})",
            resp.version,
            resp.mode
        );
        Ok(())
    }

    async fn upload_loader(&mut self, loader_data: &Bytes) -> Result<()> {
        self.state = SaharaState::WaitingDataRequest;

        self.transport.set_timeout(Duration::from_secs(2));

        let mut total_sent: u64 = 0;
        let _total_size = loader_data.len() as u64;
        let start = std::time::Instant::now();

        loop {
            // 读命令头（8 字节：cmd + len）
            let mut cmd_hdr = [0u8; 8];
            self.transport
                .read_exact(&mut cmd_hdr)
                .await
                .map_err(|e| SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("timeout waiting for command: {}", e),
                })?;

            let cmd = u32::from_le_bytes(cmd_hdr[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse command: {}", e),
            })?);

            match SaharaCommand::from_u32(cmd) {
                Some(SaharaCommand::ReadData) => {
                    let mut body = [0u8; 12];
                    self.transport.read_exact(&mut body).await?;
                    let image_id =
                        u32::from_le_bytes(body[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse image_id: {}", e),
                        })?);
                    let offset = u32::from_le_bytes(body[4..8].try_into().map_err(|e| SaharaError::TransferFailed {
                        offset: 0,
                        reason: format!("failed to parse offset: {}", e),
                    })?) as u64;
                    let length =
                        u32::from_le_bytes(body[8..12].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse length: {}", e),
                        })?) as u64;
                    tracing::trace!(image = image_id, offset = offset, length = length, "Sahara ReadData");
                    self.send_loader_chunk(loader_data, image_id, offset, length).await?;
                    total_sent += length;
                }
                Some(SaharaCommand::ReadData64) => {
                    let mut body = [0u8; 24];
                    self.transport.read_exact(&mut body).await?;
                    let image_id =
                        u64::from_le_bytes(body[0..8].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse image_id: {}", e),
                        })?);
                    let offset =
                        u64::from_le_bytes(body[8..16].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse offset: {}", e),
                        })?);
                    let length =
                        u64::from_le_bytes(body[16..24].try_into().map_err(|e| SaharaError::TransferFailed {
                            offset: 0,
                            reason: format!("failed to parse length: {}", e),
                        })?);
                    tracing::trace!(image = image_id, offset = offset, length = length, "Sahara ReadData64");
                    self.send_loader_chunk(loader_data, image_id as u32, offset, length)
                        .await?;
                    total_sent += length;
                }
                Some(SaharaCommand::EndTransfer) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    tracing::info!(
                        "Sahara loader transfer complete: {} bytes in {:.1}s ({:.1} MB/s)",
                        total_sent,
                        elapsed,
                        (total_sent as f64 / 1024.0 / 1024.0) / elapsed
                    );
                    break;
                }
                Some(SaharaCommand::CmdReady) => {
                    tracing::trace!("Sahara CMD_READY during transfer");
                }
                Some(SaharaCommand::DoneResponse) => {
                    let status = u32::from_le_bytes(cmd_hdr[4..8].try_into().unwrap_or([0; 4]));
                    tracing::debug!("Sahara DoneResponse (status={})", status);
                    break;
                }
                Some(other) => {
                    tracing::debug!(command = ?other, "Unexpected Sahara command during transfer");
                }
                None => {
                    tracing::debug!(command_id = cmd, "Unknown Sahara command ID during transfer");
                }
            }
        }

        Ok(())
    }

    async fn send_loader_chunk(&mut self, loader_data: &Bytes, image_id: u32, offset: u64, length: u64) -> Result<()> {
        if image_id != ImageId::Firehose as u32
            && image_id != ImageId::FirehoseV2 as u32
            && image_id != ImageId::Dsps as u32
            && image_id != ImageId::Apps as u32
        {
            tracing::warn!(image_id = image_id, "Unexpected image ID, proceeding anyway");
        }

        let start = offset as usize;
        let end = start + length as usize;
        if end > loader_data.len() {
            return Err(SaharaError::TransferFailed {
                offset,
                reason: format!(
                    "loader too small: need {} bytes at offset {}, file is {} bytes",
                    length,
                    offset,
                    loader_data.len()
                ),
            });
        }

        self.state = SaharaState::Transferring;
        self.transport.write_bytes(loader_data.slice(start..end)).await?;

        let total_sent = offset + length;
        let total_size = loader_data.len() as u64;
        self.emit(SaharaEvent::LoaderTransferring {
            sent: total_sent,
            total: total_size,
        });

        tracing::trace!(
            "Sahara chunk sent: {} bytes at offset {:#x} (total: {} KB)",
            length,
            offset,
            (offset + length) / 1024
        );
        Ok(())
    }

    /// 发送 Done 命令并读取设备响应
    ///
    /// DoneRequest 发完后，设备会返回一个或多个 Sahara 协议包
    /// （DoneResponse + ExecuteRequest 等），必须读取并消费掉，
    /// 否则设备不会进入 Firehose 模式。
    async fn send_done(&mut self) -> Result<()> {
        let mut done = [0u8; 8];
        done[0..4].copy_from_slice(&(SaharaCommand::DoneRequest as u32).to_le_bytes());
        done[4..8].copy_from_slice(&8u32.to_le_bytes());
        self.transport.write(&done).await?;
        tracing::trace!("Sahara DoneRequest sent");

        self.transport.set_timeout(Duration::from_secs(2));
        let mut rsp_buf = [0u8; 32];
        let n = self.transport.read(&mut rsp_buf).await?;
        if n > 0 {
            let cmd = u32::from_le_bytes(rsp_buf[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse done response: {}", e),
            })?);
            tracing::debug!("Sahara Done response: {} bytes, first_cmd=0x{:02X}", n, cmd);
        } else {
            tracing::debug!("Sahara Done response: no data received");
        }

        Ok(())
    }

    fn parse_hello(data: &[u8]) -> Result<SaharaHello> {
        if data.len() < 24 {
            return Err(SaharaError::HelloFailed);
        }
        let mode_val = u32::from_le_bytes(data[20..24].try_into().map_err(|e| SaharaError::TransferFailed {
            offset: 0,
            reason: format!("failed to parse mode: {}", e),
        })?);
        let mode = match mode_val {
            0 => SaharaMode::ImageTransfer,
            1 => SaharaMode::ImageTransferComplete,
            2 => SaharaMode::MemoryDebug,
            3 => SaharaMode::Command,
            _ => SaharaMode::ImageTransfer,
        };
        Ok(SaharaHello {
            command: SaharaCommand::Hello,
            packet_length: u32::from_le_bytes(data[4..8].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse packet_length: {}", e),
            })?),
            version: u32::from_le_bytes(data[8..12].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse version: {}", e),
            })?),
            version_min: u32::from_le_bytes(data[12..16].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse version_min: {}", e),
            })?),
            max_command_length: u32::from_le_bytes(data[16..20].try_into().map_err(|e| {
                SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to parse max_command_length: {}", e),
                }
            })?),
            mode,
            reserved: {
                let mut r = [0u32; 6];
                for (i, slot) in r.iter_mut().enumerate() {
                    let off = 24 + i * 4;
                    if off + 4 <= data.len() {
                        *slot = u32::from_le_bytes(data[off..off + 4].try_into().map_err(|e| {
                            SaharaError::TransferFailed {
                                offset: 0,
                                reason: format!("failed to parse reserved[{}]: {}", i, e),
                            }
                        })?);
                    }
                }
                r
            },
        })
    }

    pub fn state(&self) -> SaharaState {
        self.state
    }

    /// Execute a Sahara exec command and return raw response data.
    ///
    /// This sends an ExecuteRequest to the device and reads back the ExecuteData response.
    /// The device must be in Command mode for this to work.
    pub async fn exec_cmd(&mut self, client_cmd: u32) -> Result<Vec<u8>> {
        let mut pkt = [0u8; 12];
        pkt[0..4].copy_from_slice(&(SaharaCommand::ExecuteRequest as u32).to_le_bytes());
        pkt[4..8].copy_from_slice(&12u32.to_le_bytes());
        pkt[8..12].copy_from_slice(&client_cmd.to_le_bytes());
        self.transport.write(&pkt).await?;
        tracing::trace!("Sahara exec cmd=0x{:02X}", client_cmd);

        self.transport.set_timeout(Duration::from_secs(2));
        let mut hdr = [0u8; 16];
        self.transport
            .read_exact(&mut hdr)
            .await
            .map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to read exec response header: {}", e),
            })?;

        let cmd = u32::from_le_bytes(hdr[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
            offset: 0,
            reason: format!("failed to parse exec response command: {}", e),
        })?);

        if cmd == SaharaCommand::ExecuteResponse as u32 {
            let data_len = u32::from_le_bytes(hdr[12..16].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse data_len: {}", e),
            })?) as usize;

            if data_len == 0 {
                return Ok(Vec::new());
            }

            let mut data = vec![0u8; data_len];
            self.transport
                .read_exact(&mut data)
                .await
                .map_err(|e| SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to read exec data ({} bytes): {}", data_len, e),
                })?;
            Ok(data)
        } else if cmd == SaharaCommand::EndTransfer as u32 {
            let status = u32::from_le_bytes(hdr[8..12].try_into().map_err(|e| SaharaError::TransferFailed {
                offset: 0,
                reason: format!("failed to parse end transfer status: {}", e),
            })?);
            Err(SaharaError::TransferFailed {
                offset: 0,
                reason: format!("exec cmd 0x{:02X} failed with status 0x{:02X}", client_cmd, status),
            })
        } else {
            Err(SaharaError::UnexpectedCommand { cmd })
        }
    }

    /// Get MSM hardware ID (raw 8 bytes).
    ///
    /// Returns the raw MSM_HW_ID as a byte vector.
    /// The first 4 bytes are the SOC_HW_VERSION, the next 4 are typically 0.
    pub async fn get_msm_hw_id(&mut self) -> Result<Vec<u8>> {
        self.exec_cmd(exec_cmd::MSM_HW_ID_READ).await
    }

    /// Get chip serial number.
    pub async fn get_serial_num(&mut self) -> Result<u64> {
        let data = self.exec_cmd(exec_cmd::SERIAL_NUM_READ).await?;
        if data.len() >= 4 {
            Ok(u64::from_le_bytes(data[..8].try_into().map_err(|e| {
                SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to parse serial num: {}", e),
                }
            })?))
        } else {
            Err(SaharaError::TransferFailed {
                offset: 0,
                reason: format!("serial num response too short: {} bytes", data.len()),
            })
        }
    }

    /// Enter command mode, execute a function, then return transport.
    ///
    /// This is used to query device info before entering Firehose mode.
    pub async fn enter_command_mode(mut self) -> Result<Self> {
        tracing::debug!("Entering Sahara command mode");
        self.mode_switch(SaharaMode::Command).await?;

        self.transport.set_timeout(Duration::from_secs(2));
        let mut buf = [0u8; 8];
        match self.transport.read_exact(&mut buf).await {
            Ok(()) => {
                let cmd = u32::from_le_bytes(buf[0..4].try_into().map_err(|e| SaharaError::TransferFailed {
                    offset: 0,
                    reason: format!("failed to parse command: {}", e),
                })?);
                if cmd == SaharaCommand::CmdReady as u32 {
                    tracing::debug!("Sahara command mode ready");
                } else {
                    tracing::debug!("Sahara CMD_READY expected, got 0x{:08X}", cmd);
                }
            }
            Err(e) => {
                tracing::debug!("Sahara CMD_READY timeout: {}", e);
            }
        }

        Ok(self)
    }
}
