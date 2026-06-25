use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("failed to parse {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },

    #[error("image file not found: {expected}")]
    FileNotFound { expected: PathBuf },

    #[error("image too large: {image_kb} KB > partition {partition_kb} KB")]
    TooLarge { image_kb: u64, partition_kb: u64 },

    #[error("sparse parse error: {reason}")]
    SparseFailed { reason: String },

    #[error("unsupported sparse version: {version}")]
    UnsupportedSparseVersion { version: u32 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML parse error: {0}")]
    XmlParse(String),
}

pub type Result<T> = std::result::Result<T, ImageError>;

impl From<ImageError> for qedl_core::QedlError {
    fn from(e: ImageError) -> Self {
        let code = match &e {
            ImageError::ParseFailed { .. } => qedl_core::ErrorCode::ImageParseFailed,
            ImageError::FileNotFound { .. } => qedl_core::ErrorCode::ImageFileNotFound,
            ImageError::TooLarge { .. } => qedl_core::ErrorCode::ImageTooLarge,
            ImageError::SparseFailed { .. } => qedl_core::ErrorCode::ImageSparseFailed,
            ImageError::UnsupportedSparseVersion { .. } => qedl_core::ErrorCode::ImageSparseFailed,
            ImageError::Io(_) => qedl_core::ErrorCode::TransportIo,
            ImageError::XmlParse(_) => qedl_core::ErrorCode::XmlParse,
        };
        qedl_core::QedlError::image(code, e)
    }
}
