use crate::error::Result;
use qedl_core::{DeviceState, PartitionInfo, Session};
#[cfg(feature = "sparse")]
use qedl_job::VerifyJob;
use qedl_job::{
    DumpJob, EraseJob, EraseMethod, ExecutorConfig, GptJob, InfoJob, JobExecutor, JobResult, WriteJob, XmlJob,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// High-level client for Qualcomm EDL (Firehose) device communication.
pub struct QedlClient {
    executor: JobExecutor,
}

impl QedlClient {
    /// Returns a new builder for constructing a `QedlClient`.
    pub fn builder() -> QedlClientBuilder {
        QedlClientBuilder::new()
    }

    /// Creates a new client from the given executor configuration.
    pub fn from_config(config: ExecutorConfig) -> Self {
        Self {
            executor: JobExecutor::new(config),
        }
    }

    /// Returns the current device state.
    pub fn state(&self) -> DeviceState {
        self.executor.state()
    }

    /// Get session info (available after connect + handshake)
    pub fn session(&self) -> Option<&Session> {
        self.executor.session()
    }

    /// Get partition info (available after load_gpt)
    pub fn partitions(&self) -> &[PartitionInfo] {
        self.executor.partition_infos()
    }

    /// Opens a serial/USB connection to the device.
    pub fn connect(&mut self) -> Result<()> {
        self.executor.connect()?;
        Ok(())
    }

    /// Performs the Firehose handshake with the device.
    pub async fn handshake(&mut self) -> Result<()> {
        self.executor.handshake().await?;
        Ok(())
    }

    /// Initializes the Firehose protocol on the device.
    pub async fn init_firehose(&mut self) -> Result<()> {
        self.executor.init_firehose().await?;
        Ok(())
    }

    /// Loads the GPT partition table from the device.
    pub async fn load_gpt(&mut self) -> Result<()> {
        self.executor.load_gpt().await?;
        Ok(())
    }

    /// Runs the full initialization sequence: connect, handshake, configure, and load GPT.
    pub async fn init(&mut self) -> Result<()> {
        self.connect()?;
        self.handshake().await?;
        self.init_firehose().await?;
        self.load_gpt().await?;
        Ok(())
    }

    /// Initializes Firehose without loading GPT. For commands that don't need partitions.
    /// Skips Sahara handshake if the device is already in Firehose mode.
    pub async fn init_firehose_only(&mut self) -> Result<()> {
        self.executor.init_firehose_only().await?;
        Ok(())
    }

    /// Flashes raw program and patch images to the device.
    #[cfg(feature = "sparse")]
    pub async fn flash(
        &mut self,
        rawprogram: &Path,
        patch: Option<&Path>,
        image_dir: &Path,
        erase_method: EraseMethod,
    ) -> Result<JobResult> {
        let job = qedl_job::FlashJob {
            rawprogram: rawprogram.to_path_buf(),
            patch: patch.map(|p| p.to_path_buf()),
            image_dir: image_dir.to_path_buf(),
            erase_method,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Dumps a partition from the device to a local file.
    pub async fn dump(&mut self, partition: &str, output: &Path) -> Result<JobResult> {
        let job = DumpJob {
            partition_name: partition.to_string(),
            output_path: output.to_path_buf(),
            show_progress: true,
            resume: false,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Writes a local image file to a device partition.
    pub async fn write(&mut self, partition: &str, image: &Path) -> Result<JobResult> {
        let job = WriteJob {
            partition_name: partition.to_string(),
            image_path: image.to_path_buf(),
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Erases a partition on the device.
    pub async fn erase(
        &mut self,
        partition: &str,
        show_progress: bool,
        erase_method: EraseMethod,
    ) -> Result<JobResult> {
        let job = EraseJob {
            partition_name: partition.to_string(),
            show_progress,
            erase_method,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Verifies a device partition against a local image file.
    #[cfg(feature = "sparse")]
    pub async fn verify(&mut self, partition: &str, image: &Path) -> Result<JobResult> {
        let job = VerifyJob {
            partition_name: partition.to_string(),
            image_path: image.to_path_buf(),
            show_progress: true,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Sends a raw XML string to the Firehose device.
    pub async fn raw_xml(&mut self, xml: &str) -> Result<JobResult> {
        let job = XmlJob {
            xml: Some(xml.to_string()),
            file: None,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Sends an XML file to the Firehose device.
    pub async fn xml_from_file(&mut self, path: &Path) -> Result<JobResult> {
        let job = XmlJob {
            xml: None,
            file: Some(path.to_path_buf()),
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Read memory at physical address. Returns raw bytes.
    pub async fn peek(&mut self, address: u64, size: u32) -> Result<Vec<u8>> {
        self.executor.peek(address, size).await.map_err(Into::into)
    }

    /// Write memory at physical address.
    pub async fn poke(&mut self, address: u64, data: &[u8]) -> Result<()> {
        self.executor.poke(address, data).await.map_err(Into::into)
    }

    /// Dumps a partition, resuming from a previous incomplete transfer.
    pub async fn dump_resume(&mut self, partition: &str, output: &Path) -> Result<JobResult> {
        let job = DumpJob {
            partition_name: partition.to_string(),
            output_path: output.to_path_buf(),
            show_progress: true,
            resume: true,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Dumps a partition with configurable progress display and resume support.
    pub async fn dump_to(
        &mut self,
        partition: &str,
        output: &Path,
        show_progress: bool,
        resume: bool,
    ) -> Result<JobResult> {
        let job = DumpJob {
            partition_name: partition.to_string(),
            output_path: output.to_path_buf(),
            show_progress,
            resume,
        };
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Shows device information.
    pub async fn info(&mut self) -> Result<JobResult> {
        let job = InfoJob;
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Shows the device GPT partition table.
    pub async fn gpt(&mut self) -> Result<JobResult> {
        let job = GptJob;
        self.executor.execute(&job).await.map_err(Into::into)
    }

    /// Reboots the device.
    pub async fn reboot(&mut self) -> Result<()> {
        self.executor.reboot().await?;
        Ok(())
    }

    /// Returns a mutable reference to the underlying job executor.
    pub fn executor_mut(&mut self) -> &mut JobExecutor {
        &mut self.executor
    }

    /// Returns a reference to the underlying job executor.
    pub fn executor(&self) -> &JobExecutor {
        &self.executor
    }
}

/// Builder for constructing a `QedlClient` with custom configuration.
pub struct QedlClientBuilder {
    port: Option<String>,
    serial: Option<String>,
    loader: Option<std::path::PathBuf>,
    timeout: Duration,
    dry_run: bool,
    verbose: bool,
    max_retries: u32,
    event_sink: Option<Arc<dyn qedl_core::EventSink>>,
    auto_edl_switch: bool,
    spinner_factory: Option<qedl_job::SpinnerFactory>,
}

impl QedlClientBuilder {
    /// Creates a new builder with default settings.
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
            spinner_factory: None,
        }
    }

    /// Sets the serial port for device communication.
    pub fn port(mut self, port: impl Into<String>) -> Self {
        self.port = Some(port.into());
        self
    }

    /// Sets the serial number filter for device selection.
    pub fn serial(mut self, serial: impl Into<String>) -> Self {
        self.serial = Some(serial.into());
        self
    }

    /// Sets the path to the loader binary.
    pub fn loader(mut self, loader: impl Into<std::path::PathBuf>) -> Self {
        self.loader = Some(loader.into());
        self
    }

    /// Sets the operation timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enables or disables dry-run mode.
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Enables or disables verbose logging.
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Sets the maximum number of retries for failed operations.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the event sink for receiving progress and status events.
    pub fn event_sink(mut self, sink: Arc<dyn qedl_core::EventSink>) -> Self {
        self.event_sink = Some(sink);
        self
    }

    /// Enables or disables automatic DIAG to EDL (9008) mode switching.
    pub fn auto_edl_switch(mut self, switch: bool) -> Self {
        self.auto_edl_switch = switch;
        self
    }

    /// Sets the spinner factory for creating temporary spinners during long operations.
    pub fn spinner_factory(
        mut self,
        factory: impl Fn(&str) -> Box<dyn qedl_job::context::SpinnerHandle + Send> + Send + Sync + 'static,
    ) -> Self {
        self.spinner_factory = Some(Arc::new(factory));
        self
    }

    /// Builds and returns the configured `QedlClient`.
    pub fn build(self) -> QedlClient {
        let config = ExecutorConfig {
            port: self.port,
            serial: self.serial,
            loader: self.loader,
            timeout: self.timeout,
            dry_run: self.dry_run,
            verbose: self.verbose,
            max_retries: self.max_retries,
            event_sink: self.event_sink,
            auto_edl_switch: self.auto_edl_switch,
            spinner_factory: self.spinner_factory,
        };

        QedlClient::from_config(config)
    }
}

impl Default for QedlClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
