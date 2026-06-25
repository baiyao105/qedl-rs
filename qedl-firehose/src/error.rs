use thiserror::Error;

#[derive(Debug, Error)]
pub enum FirehoseError {
    #[error("configure failed: {reason}")]
    ConfigureFailed { reason: String },

    #[error("NAK for command '{command}': {reason}")]
    Nak { command: String, reason: String },

    #[error("timeout waiting for response to '{command}'")]
    Timeout { command: String },

    #[error("invalid XML response: {reason}")]
    InvalidResponse { reason: String },

    #[error("session not initialized")]
    NotInitialized,

    #[error("transport error: {0}")]
    Transport(#[from] qedl_transport::TransportError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML parse error: {0}")]
    XmlParse(String),
}

pub type Result<T> = std::result::Result<T, FirehoseError>;

impl From<FirehoseError> for qedl_core::QedlError {
    fn from(e: FirehoseError) -> Self {
        let code = match &e {
            FirehoseError::ConfigureFailed { .. } => qedl_core::ErrorCode::FirehoseConfigureFailed,
            FirehoseError::Nak { .. } => qedl_core::ErrorCode::FirehoseNak,
            FirehoseError::Timeout { .. } => qedl_core::ErrorCode::FirehoseTimeout,
            FirehoseError::InvalidResponse { .. } => qedl_core::ErrorCode::FirehoseInvalidResponse,
            FirehoseError::NotInitialized => qedl_core::ErrorCode::FirehoseNotInitialized,
            FirehoseError::Transport(_) => qedl_core::ErrorCode::TransportIo,
            FirehoseError::Io(_) => qedl_core::ErrorCode::TransportIo,
            FirehoseError::XmlParse(_) => qedl_core::ErrorCode::XmlParse,
        };
        qedl_core::QedlError::firehose(code, e)
    }
}
