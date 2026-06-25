use thiserror::Error;

#[derive(Debug, Error)]
pub enum SaharaError {
    #[error("Sahara hello handshake failed")]
    HelloFailed,

    #[error("invalid image ID: 0x{image_id:02X}")]
    InvalidImage { image_id: u8 },

    #[error("image authentication failed for ID 0x{image_id:02X}")]
    ImageAuthFailed { image_id: u8 },

    #[error("transfer failed at offset {offset:#x}: {reason}")]
    TransferFailed { offset: u64, reason: String },

    #[error("Sahara NAK: code=0x{code:02X}")]
    Nak { code: u32 },

    #[error("unexpected Sahara command: 0x{cmd:02X}")]
    UnexpectedCommand { cmd: u32 },

    #[error("protocol version mismatch: got {got}, expected {expected}")]
    VersionMismatch { got: u32, expected: u32 },

    #[error("device is already in Firehose mode, not Sahara")]
    AlreadyInFirehose,

    #[error("transport error: {0}")]
    Transport(#[from] qedl_transport::TransportError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SaharaError>;

impl From<SaharaError> for qedl_core::QedlError {
    fn from(e: SaharaError) -> Self {
        let code = match &e {
            SaharaError::HelloFailed => qedl_core::ErrorCode::SaharaHelloFailed,
            SaharaError::InvalidImage { .. } => qedl_core::ErrorCode::SaharaInvalidImage,
            SaharaError::TransferFailed { .. } => qedl_core::ErrorCode::SaharaTransferFailed,
            SaharaError::Nak { .. } => qedl_core::ErrorCode::SaharaNak,
            SaharaError::UnexpectedCommand { .. } => qedl_core::ErrorCode::SaharaUnexpectedCommand,
            SaharaError::ImageAuthFailed { .. } => qedl_core::ErrorCode::SaharaImageAuthFailed,
            SaharaError::VersionMismatch { .. } => qedl_core::ErrorCode::SaharaVersionMismatch,
            SaharaError::AlreadyInFirehose => qedl_core::ErrorCode::SaharaHelloFailed,
            SaharaError::Transport(_) => qedl_core::ErrorCode::TransportIo,
            SaharaError::Io(_) => qedl_core::ErrorCode::TransportIo,
        };
        qedl_core::QedlError::sahara(code, e)
    }
}
