use crate::context::{JobContext, XmlResponse};
use crate::error::Result;
use crate::jobs::{Job, JobResult};
use async_trait::async_trait;
use bytes::Bytes;
use qedl_core::{
    DeviceState, Event, EventSink, JobEvent, NoopProgress, PartitionInfo, ProgressReporter, Session, emit_event,
};
use qedl_firehose::FirehoseClient;
use qedl_sahara::SaharaSession;
use qedl_storage::{GptTable, PartitionMap};
use qedl_transport::{DeviceEnumerator, DeviceInfo, SerialTransport, Transport};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ExecutorConfig {
    pub port: Option<String>,
    pub serial: Option<String>,
    pub loader: Option<PathBuf>,
    pub timeout: Duration,
    pub dry_run: bool,
    pub verbose: bool,
    pub max_retries: u32,
    pub event_sink: Option<Arc<dyn EventSink>>,
    pub auto_edl_switch: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            port: None,
            serial: None,
            loader: None,
            timeout: Duration::from_secs(45),
            dry_run: false,
            verbose: false,
            max_retries: 3,
            event_sink: None,
            auto_edl_switch: true,
        }
    }
}

impl std::fmt::Debug for ExecutorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorConfig")
            .field("port", &self.port)
            .field("serial", &self.serial)
            .field("loader", &self.loader)
            .field("timeout", &self.timeout)
            .field("dry_run", &self.dry_run)
            .field("verbose", &self.verbose)
            .field("max_retries", &self.max_retries)
            .field("event_sink", &self.event_sink.as_ref().map(|_| "<EventSink>"))
            .field("auto_edl_switch", &self.auto_edl_switch)
            .finish()
    }
}

impl ExecutorConfig {
    pub fn builder() -> ExecutorConfigBuilder {
        ExecutorConfigBuilder::new()
    }
}

pub struct ExecutorConfigBuilder {
    port: Option<String>,
    serial: Option<String>,
    loader: Option<PathBuf>,
    timeout: Duration,
    dry_run: bool,
    verbose: bool,
    max_retries: u32,
    event_sink: Option<Arc<dyn EventSink>>,
    auto_edl_switch: bool,
}

impl ExecutorConfigBuilder {
    pub fn new() -> Self {
        Self {
            port: None,
            serial: None,
            loader: None,
            timeout: Duration::from_secs(45),
            dry_run: false,
            verbose: false,
            max_retries: 3,
            event_sink: None,
            auto_edl_switch: true,
        }
    }

    pub fn port(mut self, port: impl Into<String>) -> Self {
        self.port = Some(port.into());
        self
    }

    pub fn serial(mut self, serial: impl Into<String>) -> Self {
        self.serial = Some(serial.into());
        self
    }

    pub fn loader(mut self, loader: impl Into<PathBuf>) -> Self {
        self.loader = Some(loader.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.event_sink = Some(sink);
        self
    }

    pub fn auto_edl_switch(mut self, switch: bool) -> Self {
        self.auto_edl_switch = switch;
        self
    }

    pub fn build(self) -> ExecutorConfig {
        ExecutorConfig {
            port: self.port,
            serial: self.serial,
            loader: self.loader,
            timeout: self.timeout,
            dry_run: self.dry_run,
            verbose: self.verbose,
            max_retries: self.max_retries,
            event_sink: self.event_sink,
            auto_edl_switch: self.auto_edl_switch,
        }
    }
}

impl Default for ExecutorConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct JobExecutor {
    config: ExecutorConfig,
    device: Option<DeviceInfo>,
    transport: Option<Box<dyn Transport>>,
    firehose: FirehoseClient,
    partitions: PartitionMap,
    partition_infos: Vec<PartitionInfo>,
    state: DeviceState,
    session: Option<Session>,
}

impl JobExecutor {
    pub fn new(config: ExecutorConfig) -> Self {
        let firehose = FirehoseClient::new().with_event_sink(config.event_sink.clone());
        Self {
            config,
            device: None,
            transport: None,
            firehose,
            partitions: PartitionMap::new(),
            partition_infos: Vec::new(),
            state: DeviceState::Disconnected,
            session: None,
        }
    }

    pub fn emit(&self, event: JobEvent) {
        emit_event(&self.config.event_sink, Event::Job(event));
    }

    pub fn connect(&mut self) -> Result<()> {
        let device = if let Some(ref port) = self.config.port {
            tracing::debug!("Searching for device by port: {}", port);
            DeviceEnumerator::find_by_port(port)?
        } else if let Some(ref serial) = self.config.serial {
            tracing::debug!("Searching for device by serial: {}", serial);
            DeviceEnumerator::find_by_serial(serial)?
        } else {
            tracing::debug!("Auto-detecting 9008/DIAG device");
            DeviceEnumerator::auto_select()?
        };

        tracing::info!("Device found: {} (PID=0x{:04X})", device, device.pid);

        if device.is_diag() {
            if !self.config.auto_edl_switch {
                tracing::info!("Device in DIAG mode, skipping (--no-switch-edl)");
                return Err(crate::error::JobError::PreconditionFailed {
                    reason: "device is in DIAG mode, --no-switch-edl is set".to_string(),
                });
            }
            tracing::info!("Device in DIAG mode, switching to EDL (9008)");
            DeviceEnumerator::switch_diag_to_edl(&device.port, self.config.timeout.as_secs())?;
            tracing::info!("DIAG -> EDL switch successful");
            let device = if let Some(ref port) = self.config.port {
                DeviceEnumerator::find_by_port(port)?
            } else {
                DeviceEnumerator::auto_select()?
            };
            tracing::info!("Device after switch: {} (PID=0x{:04X})", device, device.pid);
            let switched_port = device.port.clone();
            let serial = device.serial.clone();
            let product = device.product.clone();
            let description = device.description.clone();
            self.device = Some(device);
            self.state = DeviceState::Connected;
            self.session = Some(Session::new(
                qedl_core::DeviceInfo {
                    port: switched_port,
                    serial,
                    product: product.clone(),
                    pid: 0x9008,
                    vid: 0x05C6,
                    description,
                    mode: qedl_core::DeviceMode::Edl,
                },
                qedl_core::DeviceCapabilities::default(),
                qedl_core::FirehoseInfo::default(),
            ));
            return Ok(());
        }

        let port = device.port.clone();
        let serial = device.serial.clone();
        let product = device.product.clone();
        let pid = device.pid;
        let vid = device.vid;
        let description = device.description.clone();
        let mode = device.mode;
        self.device = Some(device);
        self.state = DeviceState::Connected;
        self.session = Some(Session::new(
            qedl_core::DeviceInfo {
                port,
                serial,
                product,
                pid,
                vid,
                description,
                mode,
            },
            qedl_core::DeviceCapabilities::default(),
            qedl_core::FirehoseInfo::default(),
        ));
        Ok(())
    }

    pub async fn handshake(&mut self) -> Result<()> {
        let loader_path = self
            .config
            .loader
            .as_ref()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "no loader specified (--loader)".to_string(),
            })?;

        let device = self
            .device
            .as_ref()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "not connected".to_string(),
            })?;

        tracing::info!("Opening serial port {}", device.port);
        let transport = SerialTransport::open(&device.port, 115200, self.config.timeout)?;

        tracing::info!("Starting Sahara handshake");
        let sahara = SaharaSession::new(transport).with_event_sink(self.config.event_sink.clone());
        match sahara
            .handshake(loader_path, qedl_sahara::SaharaMode::ImageTransfer)
            .await
        {
            Ok((transport, sahara_info)) => {
                tracing::info!("Sahara handshake complete");
                if let Some(ref mut session) = self.session {
                    session.msm_hw_id = sahara_info.msm_hw_id;
                    session.serial_num = sahara_info.serial_num;
                }
                self.transport = Some(Box::new(transport));
                self.state = DeviceState::Ready;
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    pub async fn init_firehose(&mut self) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;

        self.firehose.drain_initial_messages(transport.as_mut()).await?;

        tracing::info!("Configuring Firehose");
        self.firehose.configure(transport.as_mut()).await?;
        tracing::info!(
            "Firehose configured (Target={} Memory={})",
            self.firehose.target_name,
            self.firehose.memory_name
        );

        tracing::debug!("Querying storage info");
        match self.firehose.get_storage_info(transport.as_mut()).await {
            Ok(storage_resp) => {
                if let Some(name) = &storage_resp.memory_name {
                    tracing::debug!("Storage type: {}", name);
                }
                if let Some(ss) = storage_resp.sector_size {
                    tracing::debug!("Sector size: {} bytes", ss);
                    self.firehose.sector_size = ss;
                }
                if let Some(ts) = storage_resp.total_sectors {
                    tracing::debug!("Total sectors: {}", ts);
                    self.firehose.total_sectors = ts;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "getstorageinfo failed, continuing");
            }
        }

        if let Some(ref mut session) = self.session {
            session.firehose.sector_size = self.firehose.sector_size;
            session.firehose.max_payload_size = self.firehose.max_payload_size;
            session.firehose.max_payload_size_from_target = self.firehose.max_payload_size_from_target;
            session.firehose.max_payload_size_to_target_supported = self.firehose.max_payload_size_to_target_supported;
            session.firehose.max_xml_size = self.firehose.max_xml_size;
            session.firehose.target_name = Some(self.firehose.target_name.clone());
            session.firehose.version = self.firehose.version.clone();
            session.capabilities.memory_type = self.firehose.memory_name.clone();
            session.capabilities.total_sectors = self.firehose.total_sectors;
        }

        Ok(())
    }

    pub async fn load_gpt(&mut self) -> Result<()> {
        tracing::info!("Reading GPT partition table");
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;

        let sector_size = self.firehose.sector_size();
        let total_sectors = self.firehose.total_sectors;

        let max_lun = if self.firehose.memory_name.to_lowercase().contains("ufs") {
            tracing::debug!("UFS storage detected, scanning LUNs 0-3");
            4u8
        } else {
            tracing::debug!("eMMC storage detected, scanning LUN 0 only");
            1u8
        };

        let luns: Vec<u8> = (0..max_lun).collect();
        for &lun in &luns {
            tracing::debug!("Reading GPT from LUN {}", lun);
            match read_gpt_for_lun(transport.as_mut(), &mut self.firehose, lun, sector_size, total_sectors).await {
                Ok(table) => {
                    if self.firehose.total_sectors == 0
                        && table.primary_valid
                        && let Some(hdr) = &table.header
                    {
                        let calculated_total = hdr.backup_lba + 1;
                        tracing::debug!("Using GPT backup_lba for total sectors: {}", calculated_total);
                        self.firehose.total_sectors = calculated_total;
                    }

                    let count = table.entries.len();
                    for entry in &table.entries {
                        let clean_name = entry.name.trim().trim_matches('\0').trim();
                        tracing::trace!(
                            "Partition: name='{}', LBA={}..{}, LUN={}",
                            clean_name,
                            entry.first_lba,
                            entry.last_lba,
                            lun
                        );
                    }
                    self.partitions.add_table(table);
                    tracing::debug!("LUN {}: {} partitions loaded", lun, count);
                }
                Err(e) => {
                    tracing::trace!(error = %e, "LUN {}: no valid GPT", lun);
                    if lun == 0 {
                        return Err(e);
                    }
                    continue;
                }
            }
        }

        self.partition_infos = self
            .partitions
            .all_entries()
            .into_iter()
            .map(|e| PartitionInfo {
                name: e.name.clone(),
                first_lba: e.first_lba,
                last_lba: e.last_lba,
                physical_partition: e.physical_partition,
            })
            .collect();

        tracing::info!("Found {} partitions", self.partitions.total_partitions());
        Ok(())
    }

    pub async fn execute(&mut self, job: &dyn Job) -> Result<JobResult> {
        if self.config.dry_run {
            tracing::info!("Dry-run mode, skipping execution");
            return Ok(JobResult {
                success: true,
                message: "dry-run: parsed successfully".to_string(),
                steps_completed: 0,
            });
        }

        if self.transport.is_none() {
            return Err(crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            });
        }

        job.execute(self).await
    }

    pub fn device(&self) -> Option<&DeviceInfo> {
        self.device.as_ref()
    }

    pub fn partitions(&self) -> &PartitionMap {
        &self.partitions
    }

    pub fn firehose(&self) -> &FirehoseClient {
        &self.firehose
    }

    pub fn state(&self) -> DeviceState {
        self.state
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn partition_infos(&self) -> &[PartitionInfo] {
        &self.partition_infos
    }

    pub async fn reboot(&mut self) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose.reboot(transport.as_mut()).await?;
        self.state = DeviceState::Resetting;
        Ok(())
    }

    /// Read memory at physical address. Returns raw bytes.
    pub async fn peek(&mut self, address: u64, size: u32) -> Result<Vec<u8>> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose
            .peek(transport.as_mut(), address, size)
            .await
            .map_err(Into::into)
    }

    /// Write memory at physical address.
    pub async fn poke(&mut self, address: u64, data: &[u8]) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose
            .poke(transport.as_mut(), address, data)
            .await
            .map_err(Into::into)
    }

    pub async fn init(&mut self) -> Result<()> {
        self.connect()?;
        self.handshake().await?;
        self.init_firehose().await?;
        self.load_gpt().await?;
        Ok(())
    }

    /// Connect + Firehose init, skipping Sahara handshake if device is already in Firehose mode.
    pub async fn init_firehose_only(&mut self) -> Result<()> {
        self.connect()?;

        let device = self
            .device
            .as_ref()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "not connected".to_string(),
            })?;

        tracing::info!("Opening serial port {}", device.port);
        let transport = SerialTransport::open(&device.port, 115200, self.config.timeout)?;

        let mut boxed: Box<dyn Transport> = Box::new(transport);
        tracing::info!("Attempting direct Firehose configure (skip Sahara)");
        self.firehose.drain_initial_messages(boxed.as_mut()).await?;
        match self.firehose.configure(boxed.as_mut()).await {
            Ok(()) => {
                tracing::info!("Device already in Firehose mode");
                self.transport = Some(boxed);
                self.state = DeviceState::Ready;
            }
            Err(e) => {
                tracing::info!("Direct configure failed ({}), trying Sahara handshake", e);
                drop(boxed);
                self.handshake().await?;
                self.init_firehose().await?;
                return Ok(());
            }
        }

        tracing::debug!("Querying storage info");
        match self
            .firehose
            .get_storage_info(self.transport.as_mut().unwrap().as_mut())
            .await
        {
            Ok(storage_resp) => {
                if let Some(name) = &storage_resp.memory_name {
                    tracing::debug!("Storage type: {}", name);
                }
                if let Some(ss) = storage_resp.sector_size {
                    tracing::debug!("Sector size: {} bytes", ss);
                    self.firehose.sector_size = ss;
                }
                if let Some(ts) = storage_resp.total_sectors {
                    tracing::debug!("Total sectors: {}", ts);
                    self.firehose.total_sectors = ts;
                }
                if let Some(ref mut session) = self.session {
                    session.firehose.sector_size = self.firehose.sector_size;
                    session.firehose.max_payload_size = self.firehose.max_payload_size;
                    session.capabilities.memory_type = self.firehose.memory_name.clone();
                    session.capabilities.total_sectors = self.firehose.total_sectors;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "getstorageinfo failed, continuing");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl JobContext for JobExecutor {
    async fn read_sectors(&mut self, physical_partition: u8, start_sector: u64, num_sectors: u64) -> Result<Bytes> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        Ok(self
            .firehose
            .read_sectors(transport.as_mut(), physical_partition, start_sector, num_sectors)
            .await?)
    }

    async fn write_sectors(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose
            .program_sectors(transport.as_mut(), physical_partition, start_sector, num_sectors, data)
            .await?;
        Ok(())
    }

    fn sector_size(&self) -> u32 {
        self.firehose.sector_size()
    }

    fn max_payload_size(&self) -> u32 {
        self.firehose.max_payload_size()
    }

    fn storage_name(&self) -> &str {
        &self.firehose.memory_name
    }

    fn total_sectors(&self) -> u64 {
        self.firehose.total_sectors
    }

    fn find_partition(&self, name: &str) -> Option<&PartitionInfo> {
        self.partition_infos.iter().find(|p| p.name == name)
    }

    fn all_partitions(&self) -> Vec<&PartitionInfo> {
        self.partition_infos.iter().collect()
    }

    async fn reboot(&mut self) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose.reboot(transport.as_mut()).await?;
        self.state = DeviceState::Resetting;
        Ok(())
    }

    async fn raw_xml(&mut self, xml: &str) -> Result<XmlResponse> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        let resp = self.firehose.raw_xml(transport.as_mut(), xml).await?;
        Ok(XmlResponse {
            is_ack: resp.is_ack(),
            error: resp.error,
        })
    }

    async fn refresh_storage_info(&mut self) -> Result<Vec<String>> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        let resp = self.firehose.get_storage_info(transport.as_mut()).await?;
        if let Some(memory_name) = resp.memory_name {
            self.firehose.memory_name = memory_name;
        }
        if let Some(sector_size) = resp.sector_size {
            self.firehose.sector_size = sector_size;
        }
        if let Some(total_sectors) = resp.total_sectors {
            self.firehose.total_sectors = total_sectors;
        }
        Ok(resp.logs)
    }

    async fn get_sha256_digest(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<String> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose
            .get_sha256_digest(transport.as_mut(), physical_partition, start_sector, num_sectors)
            .await
            .map_err(Into::into)
    }

    async fn erase_sectors_native(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                reason: "transport not initialized".to_string(),
            })?;
        self.firehose
            .erase_sectors(transport.as_mut(), physical_partition, start_sector, num_sectors)
            .await?;
        Ok(())
    }

    fn progress(&self) -> &dyn ProgressReporter {
        static NOOP: NoopProgress = NoopProgress;
        &NOOP
    }

    fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }
}

async fn read_gpt_for_lun(
    transport: &mut dyn Transport,
    firehose: &mut FirehoseClient,
    lun: u8,
    sector_size: u32,
    total_sectors: u64,
) -> Result<GptTable> {
    tracing::trace!("Reading GPT header from LUN {} LBA 1", lun);
    let lba1_data = firehose.read_sectors(transport, lun, 1, 1).await?;

    let (gpt, header_source) = match GptTable::parse(&lba1_data, &[], lun, sector_size) {
        Ok(g) => {
            tracing::trace!("LUN {}: Primary GPT header valid", lun);
            (g, lba1_data.clone())
        }
        Err(e) => {
            tracing::trace!("LUN {}: Primary GPT invalid ({}), trying backup", lun, e);
            if total_sectors == 0 {
                tracing::debug!("LUN {}: cannot use backup GPT (total_sectors=0)", lun);
                return Err(e.into());
            }
            let backup_lba = total_sectors - 1;
            tracing::trace!("Reading backup GPT header at LBA {}", backup_lba);
            let backup_header = firehose.read_sectors(transport, lun, backup_lba, 1).await?;
            let g = GptTable::parse(&backup_header, &[], lun, sector_size)?;
            (g, backup_header)
        }
    };

    let entry_sectors = gpt.header.as_ref().map_or(0, |h| {
        (h.num_partition_entries * h.partition_entry_size).div_ceil(sector_size)
    }) as u64;

    let entry_lba = gpt.header.as_ref().map_or(2, |h| h.partition_entry_start_lba);

    if entry_sectors > 0 {
        tracing::trace!(
            "Reading {} GPT entries at LBA {}",
            gpt.header.as_ref().map_or(0, |h| h.num_partition_entries),
            entry_lba
        );
        let entries_data = firehose.read_sectors(transport, lun, entry_lba, entry_sectors).await?;
        let gpt = GptTable::parse(&header_source, &entries_data, lun, sector_size)?;

        if total_sectors > 0 {
            let backup_lba = total_sectors - 1;
            let backup_entries_lba = backup_lba - entry_sectors;
            match firehose
                .read_sectors(transport, lun, backup_entries_lba, entry_sectors)
                .await
            {
                Ok(backup_entries) => {
                    let backup_valid = !backup_entries.is_empty() && backup_entries.len() == entries_data.len();
                    if !backup_valid {
                        tracing::trace!("LUN {}: Backup GPT entries mismatch", lun);
                    }
                }
                Err(e) => {
                    tracing::trace!("LUN {}: backup GPT verification failed: {}", lun, e);
                }
            }
        }

        return Ok(gpt);
    }

    tracing::debug!("LUN {}: GPT header present but entries count is 0", lun);
    Ok(gpt)
}
