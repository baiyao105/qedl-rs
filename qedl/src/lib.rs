//! qedl: Qualcomm EDL SDK
//!
//! A high-level Rust SDK for communicating with Qualcomm devices in EDL (Emergency Download) mode.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use qedl::{EraseMethod, QedlClient};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), qedl::QedlFacadeError> {
//!     let mut client = QedlClient::builder()
//!         .port("COM3")
//!         .build();
//!
//!     client.init().await?;
//!     client.flash(Path::new("rawprogram.xml"), None, Path::new("./images"), EraseMethod::WriteZero).await?;
//!     client.reboot().await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;

pub use qedl_core as core_types;
pub use qedl_firehose as firehose;
pub use qedl_job as job;
pub use qedl_storage as storage;
pub use qedl_transport as transport;

#[cfg(feature = "sahara")]
pub use qedl_sahara as sahara;

#[cfg(feature = "sparse")]
pub use qedl_image as image;

pub use client::{QedlClient, QedlClientBuilder, QedlClientTrait};
pub use error::{QedlFacadeError, Result as QedlResult};

pub use qedl_core::{
    DeviceCapabilities, DeviceInfo, DeviceMode, DeviceState, ErrorCode, EventSink, FirehoseInfo, NoopEventSink,
    NoopProgress, PartitionInfo, ProgressReporter, Session,
};
pub use qedl_firehose::FirehoseError;
pub use qedl_job::{
    EraseMethod, ExecutorConfig, ExecutorConfigBuilder, Job, JobContext, JobError, JobExecutor, JobResult,
    ProgressFactory, SpinnerFactory, SpinnerHandle,
};
pub use qedl_storage::StorageError;
pub use qedl_transport::{DeviceEnumerator, DeviceEnumeratorTrait, MockTransport, Transport, TransportError};

#[cfg(feature = "sahara")]
pub use qedl_sahara::{SaharaCommand, SaharaError, SaharaMode, SaharaSession};
