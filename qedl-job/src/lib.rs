//! Job orchestration for flash, dump, erase, and info operations.

pub mod context;
pub mod error;
pub mod executor;
pub mod jobs;
pub mod reader;

pub use context::SpinnerHandle;
pub use context::{JobContext, XmlResponse};
pub use error::{JobError, Result};
pub use executor::{ExecutorConfig, ExecutorConfigBuilder, JobExecutor, ProgressFactory, SpinnerFactory};
pub use jobs::{DumpJob, EraseJob, EraseMethod, GptJob, InfoJob, Job, JobResult, RebootJob, WriteJob, XmlJob};
#[cfg(feature = "sparse")]
pub use jobs::{FlashJob, VerifyJob};
pub use reader::ChunkedReader;

pub mod testutil;
