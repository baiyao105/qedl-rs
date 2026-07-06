use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    TransportNotFound,
    TransportTimeout,
    TransportDisconnected,
    TransportIo,

    SaharaHelloFailed,
    SaharaInvalidImage,
    SaharaImageAuthFailed,
    SaharaTransferFailed,
    SaharaNak,
    SaharaUnexpectedCommand,
    SaharaVersionMismatch,

    FirehoseConfigureFailed,
    FirehoseNak,
    FirehoseTimeout,
    FirehoseInvalidResponse,
    FirehoseNotInitialized,

    StorageInvalidSignature,
    StorageCrcMismatch,
    StoragePartitionNotFound,
    StorageEmptyTable,

    ImageParseFailed,
    ImageFileNotFound,
    ImageTooLarge,
    ImageSparseFailed,

    JobStepFailed,
    JobPreconditionFailed,

    LoaderNotFound,
    InvalidArgument,

    XmlParse,

    /// Custom error code for external extensions. (code, display_name)
    Custom(u32, &'static str),
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TransportNotFound => "TRANSPORT_NOT_FOUND",
            Self::TransportTimeout => "TRANSPORT_TIMEOUT",
            Self::TransportDisconnected => "TRANSPORT_DISCONNECTED",
            Self::TransportIo => "TRANSPORT_IO",
            Self::SaharaHelloFailed => "SAHARA_HELLO_FAILED",
            Self::SaharaInvalidImage => "SAHARA_INVALID_IMAGE",
            Self::SaharaImageAuthFailed => "SAHARA_IMAGE_AUTH_FAILED",
            Self::SaharaTransferFailed => "SAHARA_TRANSFER_FAILED",
            Self::SaharaNak => "SAHARA_NAK",
            Self::SaharaUnexpectedCommand => "SAHARA_UNEXPECTED_COMMAND",
            Self::SaharaVersionMismatch => "SAHARA_VERSION_MISMATCH",
            Self::FirehoseConfigureFailed => "FIREHOSE_CONFIGURE_FAILED",
            Self::FirehoseNak => "FIREHOSE_NAK",
            Self::FirehoseTimeout => "FIREHOSE_TIMEOUT",
            Self::FirehoseInvalidResponse => "FIREHOSE_INVALID_RESPONSE",
            Self::FirehoseNotInitialized => "FIREHOSE_NOT_INITIALIZED",
            Self::StorageInvalidSignature => "STORAGE_INVALID_SIGNATURE",
            Self::StorageCrcMismatch => "STORAGE_CRC_MISMATCH",
            Self::StoragePartitionNotFound => "STORAGE_PARTITION_NOT_FOUND",
            Self::StorageEmptyTable => "STORAGE_EMPTY_TABLE",
            Self::ImageParseFailed => "IMAGE_PARSE_FAILED",
            Self::ImageFileNotFound => "IMAGE_FILE_NOT_FOUND",
            Self::ImageTooLarge => "IMAGE_TOO_LARGE",
            Self::ImageSparseFailed => "IMAGE_SPARSE_FAILED",
            Self::JobStepFailed => "JOB_STEP_FAILED",
            Self::JobPreconditionFailed => "JOB_PRECONDITION_FAILED",
            Self::LoaderNotFound => "LOADER_NOT_FOUND",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::XmlParse => "XML_PARSE",
            Self::Custom(_, name) => name,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QedlError {
    #[error("{code}: {message}")]
    WithCode { code: ErrorCode, message: String },

    #[error("transport not found: {0}")]
    TransportNotFound(String),

    #[error("transport timeout: {0}")]
    TransportTimeout(String),

    #[error("transport disconnected: {0}")]
    TransportDisconnected(String),

    #[error("sahara error: {0}")]
    Sahara(String),

    #[error("firehose error: {0}")]
    Firehose(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("image error: {0}")]
    Image(String),

    #[error("job error: {0}")]
    Job(String),

    #[error("{0}")]
    Other(String),
}

impl QedlError {
    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            Self::WithCode { code, .. } => Some(*code),
            Self::TransportNotFound(_) => Some(ErrorCode::TransportNotFound),
            Self::TransportTimeout(_) => Some(ErrorCode::TransportTimeout),
            Self::TransportDisconnected(_) => Some(ErrorCode::TransportDisconnected),
            Self::Sahara(_) => None,
            Self::Firehose(_) => None,
            Self::Storage(_) => None,
            Self::Image(_) => None,
            Self::Job(_) => None,
            Self::Other(_) => None,
        }
    }

    pub fn transport(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }

    pub fn sahara(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }

    pub fn firehose(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }

    pub fn storage(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }

    pub fn image(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }

    pub fn job(code: ErrorCode, e: impl fmt::Display) -> Self {
        Self::WithCode {
            code,
            message: e.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, QedlError>;
