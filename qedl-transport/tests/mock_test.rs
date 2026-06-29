use qedl_transport::MockTransport;
use qedl_transport::Transport;
use std::io;
use std::time::Duration;

#[tokio::test]
async fn test_push_and_read() {
    let mut t = MockTransport::new();
    t.push_read_data(&[1, 2, 3]);

    let mut buf = [0u8; 10];
    let n = t.read(&mut buf).await.unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..3], &[1, 2, 3]);
}

#[tokio::test]
async fn test_read_empty_queue() {
    let mut t = MockTransport::new();
    let mut buf = [0u8; 10];
    let n = t.read(&mut buf).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn test_write_log() {
    let mut t = MockTransport::new();
    t.write(&[10, 20, 30]).await.unwrap();
    t.write(&[40, 50]).await.unwrap();

    assert_eq!(t.written_data().len(), 2);
    assert_eq!(t.bytes_written(), 5);
    assert_eq!(t.written_data()[0].as_ref(), &[10, 20, 30]);
    assert_eq!(t.written_data()[1].as_ref(), &[40, 50]);
}

#[tokio::test]
async fn test_clear_write_log() {
    let mut t = MockTransport::new();
    t.write(&[1]).await.unwrap();
    assert_eq!(t.bytes_written(), 1);
    t.clear_write_log();
    assert_eq!(t.bytes_written(), 0);
}

#[tokio::test]
async fn test_read_error_after_n() {
    let mut t = MockTransport::new().with_read_error_after(2, io::ErrorKind::ConnectionReset);
    t.push_read_data(&[1]);
    t.push_read_data(&[2]);
    t.push_read_data(&[3]); // this should trigger error

    let mut buf = [0u8; 10];
    t.read(&mut buf).await.unwrap(); // read 1
    t.read(&mut buf).await.unwrap(); // read 2
    let result = t.read(&mut buf).await; // read 3 → error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_write_error_after_n() {
    let mut t = MockTransport::new().with_write_error_after(1, io::ErrorKind::BrokenPipe);
    t.write(&[1]).await.unwrap(); // ok
    let result = t.write(&[2]).await; // error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_disconnect_after_reads() {
    let mut t = MockTransport::new().with_disconnect_after_reads(1);
    t.push_read_data(&[1]);
    t.push_read_data(&[2]);

    let mut buf = [0u8; 10];
    t.read(&mut buf).await.unwrap(); // read 1
    let result = t.read(&mut buf).await; // read 2 → disconnect
    assert!(result.is_err());
}

#[tokio::test]
async fn test_corrupt_after_reads() {
    let mut t = MockTransport::new().with_corrupt_after_reads(1, 0xFF);
    t.push_read_data(&[0x00, 0x00, 0x00]);

    let mut buf = [0u8; 3];
    t.read(&mut buf).await.unwrap();
    assert_eq!(buf[0], 0xFF); // corrupted
    assert_eq!(buf[1], 0x00);
}

#[tokio::test]
async fn test_push_reads_batch() {
    let mut t = MockTransport::new();
    t.push_reads(vec![
        bytes::Bytes::from_static(&[1]),
        bytes::Bytes::from_static(&[2]),
        bytes::Bytes::from_static(&[3]),
    ]);

    let mut buf = [0u8; 10];
    let n1 = t.read(&mut buf).await.unwrap();
    assert_eq!(n1, 1);
    assert_eq!(buf[0], 1);
    let n2 = t.read(&mut buf).await.unwrap();
    assert_eq!(n2, 1);
    assert_eq!(buf[0], 2);
    let n3 = t.read(&mut buf).await.unwrap();
    assert_eq!(n3, 1);
    assert_eq!(buf[0], 3);
}

#[tokio::test]
async fn test_reset() {
    let mut t = MockTransport::new();
    t.push_read_data(&[1]);
    t.write(&[2]).await.unwrap();
    t.reset();

    let mut buf = [0u8; 10];
    let n = t.read(&mut buf).await.unwrap();
    assert_eq!(n, 0);
    assert_eq!(t.bytes_written(), 0);
}

#[tokio::test]
async fn test_timeout() {
    let mut t = MockTransport::new();
    assert_eq!(t.timeout(), Duration::from_secs(30));
    t.set_timeout(Duration::from_secs(5));
    assert_eq!(t.timeout(), Duration::from_secs(5));
}

#[tokio::test]
async fn test_flush() {
    let mut t = MockTransport::new();
    assert!(t.flush().await.is_ok());
}

#[tokio::test]
async fn test_read_exact_ok() {
    let mut t = MockTransport::new();
    t.push_read_data(&[1, 2, 3, 4, 5]);

    let mut buf = [0u8; 5];
    t.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, &[1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn test_read_exact_eof() {
    let mut t = MockTransport::new();
    // empty queue → immediate EOF

    let mut buf = [0u8; 5];
    let result = t.read_exact(&mut buf).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_default() {
    let t = MockTransport::default();
    assert_eq!(t.timeout(), Duration::from_secs(30));
}
