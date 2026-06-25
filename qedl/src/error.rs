use thiserror::Error;

/// Unified error type for qedl SDK operations
#[derive(Debug, Error)]
pub enum QedlFacadeError {
    #[error("device not connected")]
    NotConnected,

    #[error("device not ready: {0}")]
    NotReady(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("transport error: {0}")]
    Transport(#[from] qedl_transport::TransportError),

    #[cfg(feature = "sahara")]
    #[error("sahara error: {0}")]
    Sahara(#[from] qedl_sahara::SaharaError),

    #[error("firehose error: {0}")]
    Firehose(#[from] qedl_firehose::FirehoseError),

    #[error("storage error: {0}")]
    Storage(#[from] qedl_storage::StorageError),

    #[cfg(feature = "sparse")]
    #[error("image error: {0}")]
    Image(#[from] qedl_image::ImageError),

    #[error("job error: {0}")]
    Job(#[from] qedl_job::JobError),

    #[error("core error: {0}")]
    Core(#[from] qedl_core::QedlError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, QedlFacadeError>;
