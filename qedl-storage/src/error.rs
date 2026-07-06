//! Storage layer error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("invalid GPT signature: expected 'EFI PART', got {got:?}")]
    InvalidSignature { got: String },

    #[error("GPT CRC mismatch (primary={primary}): expected {expected:#010x}, got {actual:#010x}")]
    CrcMismatch { primary: bool, expected: u32, actual: u32 },

    #[error("partition '{name}' not found")]
    PartitionNotFound { name: String },

    #[error("partition table is empty")]
    EmptyPartitionTable,

    #[error("invalid partition entry at index {index}")]
    InvalidEntry { index: usize },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

impl From<StorageError> for qedl_core::QedlError {
    fn from(e: StorageError) -> Self {
        let code = match &e {
            StorageError::InvalidSignature { .. } => qedl_core::ErrorCode::StorageInvalidSignature,
            StorageError::CrcMismatch { .. } => qedl_core::ErrorCode::StorageCrcMismatch,
            StorageError::PartitionNotFound { .. } => qedl_core::ErrorCode::StoragePartitionNotFound,
            StorageError::EmptyPartitionTable => qedl_core::ErrorCode::StorageEmptyTable,
            StorageError::InvalidEntry { .. } => qedl_core::ErrorCode::StorageInvalidSignature,
            StorageError::Io(_) => qedl_core::ErrorCode::TransportIo,
        };
        qedl_core::QedlError::storage(code, e)
    }
}
