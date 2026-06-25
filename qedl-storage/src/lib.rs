//! GPT parsing and partition operations.

pub mod error;
pub mod gpt;
pub mod partition;

pub use error::StorageError;
pub use gpt::{GptEntry, GptTable};
pub use partition::PartitionMap;
