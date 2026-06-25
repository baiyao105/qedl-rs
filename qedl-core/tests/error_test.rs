use qedl_core::{ErrorCode, QedlError};

#[tokio::test]
async fn test_error_code_preserved() {
    let err = QedlError::transport(ErrorCode::TransportNotFound, "device not found");
    assert_eq!(err.code(), Some(ErrorCode::TransportNotFound));
}

#[tokio::test]
async fn test_io_error_conversion() {
    let err = QedlError::transport(ErrorCode::TransportIo, "serial port I/O error");
    assert_eq!(err.code(), Some(ErrorCode::TransportIo));
}

#[tokio::test]
async fn test_error_display() {
    let err = QedlError::firehose(ErrorCode::FirehoseNak, "command rejected");
    assert!(err.to_string().contains("command rejected"));
}
