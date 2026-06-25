//! qedl: Qualcomm EDL SDK
//!
//! A high-level Rust SDK for communicating with Qualcomm devices in EDL (Emergency Download) mode.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use qedl::QedlClient;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), qedl::QedlFacadeError> {
//!     let mut client = QedlClient::builder()
//!         .port("COM3")
//!         .build();
//!
//!     client.init().await?;
//!     client.flash(Path::new("rawprogram.xml"), None, Path::new("./images")).await?;
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

pub use client::{QedlClient, QedlClientBuilder};
pub use error::{QedlFacadeError, Result as QedlResult};

pub use qedl_core::{DeviceState, ErrorCode, PartitionInfo, Session};
pub use qedl_job::{ExecutorConfig, JobExecutor, JobResult};

#[cfg(feature = "sahara")]
pub use qedl_sahara::{SaharaError, SaharaSession};
