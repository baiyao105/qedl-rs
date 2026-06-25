use std::io;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("no 9008 device found")]
    NotFound,

    #[error("multiple devices found ({count}), please specify --serial")]
    MultipleFound { count: usize },

    #[error("serial port timeout after {elapsed:?}")]
    Timeout { elapsed: Duration },

    #[error("device disconnected")]
    Disconnected,

    #[error("serial port I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid port: {0}")]
    InvalidPort(String),
}

pub type Result<T> = std::result::Result<T, TransportError>;

impl From<TransportError> for qedl_core::QedlError {
    fn from(e: TransportError) -> Self {
        let code = match &e {
            TransportError::NotFound => qedl_core::ErrorCode::TransportNotFound,
            TransportError::Timeout { .. } => qedl_core::ErrorCode::TransportTimeout,
            TransportError::Disconnected => qedl_core::ErrorCode::TransportDisconnected,
            TransportError::Io(_) => qedl_core::ErrorCode::TransportIo,
            _ => qedl_core::ErrorCode::TransportIo,
        };
        qedl_core::QedlError::transport(code, e)
    }
}
