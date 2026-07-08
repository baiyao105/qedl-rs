//! GPT parsing and partition operations.
//!
//! Parses GUID Partition Tables from sector data, supports both eMMC and UFS
//! layouts, and provides [`PartitionMap`] for looking up partitions by name.

pub mod error;
pub mod gpt;
pub mod partition;

pub use error::StorageError;
pub use gpt::{GptEntry, GptTable};
pub use partition::PartitionMap;
