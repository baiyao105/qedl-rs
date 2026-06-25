//! Job orchestration for flash, dump, erase, and info operations.

pub mod context;
pub mod error;
pub mod executor;
pub mod jobs;
pub mod parser;
pub mod reader;

pub use context::{JobContext, XmlResponse};
pub use error::{JobError, Result};
pub use executor::{ExecutorConfig, JobExecutor};
pub use jobs::{DumpJob, EraseJob, GptJob, InfoJob, Job, JobResult, RebootJob, WriteJob, XmlJob};
#[cfg(feature = "sparse")]
pub use jobs::{FlashJob, VerifyJob};
pub use parser::{ParseError, RawEntry, RawProgram};
pub use reader::ChunkedReader;
