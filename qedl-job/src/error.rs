use thiserror::Error;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("step {step} failed: {reason}")]
    StepFailed { step: usize, reason: String },

    #[error("precondition failed: {reason}")]
    PreconditionFailed { reason: String },

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

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, JobError>;

impl From<JobError> for qedl_core::QedlError {
    fn from(e: JobError) -> Self {
        let code = match &e {
            JobError::StepFailed { .. } => qedl_core::ErrorCode::JobStepFailed,
            JobError::PreconditionFailed { .. } => qedl_core::ErrorCode::JobPreconditionFailed,
            JobError::Transport(_) => qedl_core::ErrorCode::TransportIo,
            #[cfg(feature = "sahara")]
            JobError::Sahara(_) => qedl_core::ErrorCode::SaharaHelloFailed,
            JobError::Firehose(_) => qedl_core::ErrorCode::FirehoseNak,
            JobError::Storage(_) => qedl_core::ErrorCode::StoragePartitionNotFound,
            #[cfg(feature = "sparse")]
            JobError::Image(_) => qedl_core::ErrorCode::ImageParseFailed,
            JobError::Io(_) => qedl_core::ErrorCode::TransportIo,
        };
        qedl_core::QedlError::job(code, e)
    }
}
