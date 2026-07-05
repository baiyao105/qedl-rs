use qedl_firehose::FirehoseClient;
use qedl_transport::{MockTransport, Transport};
use std::time::Duration;

fn ack_response() -> Vec<u8> {
    br#"<?xml version="1.0" encoding="UTF-8" ?><data><response value="ACK" /></data>"#.to_vec()
}

fn nak_response(reason: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" ?><data><response value="NAK"><log value="{}" /></response></data>"#,
        reason
    )
    .into_bytes()
}

#[tokio::test]
async fn test_configure_ack() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let result = client.configure(&mut t).await;
    assert!(result.is_ok());
    assert!(client.is_initialized());
}

#[tokio::test]
async fn test_configure_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("unsupported"));

    let mut client = FirehoseClient::new();
    let result = client.configure(&mut t).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_reboot_ack() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let result = client.reboot(&mut t).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reboot_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("not allowed"));

    let mut client = FirehoseClient::new();
    let result = client.reboot(&mut t).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_erase_sectors_ack() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let result = client.erase_sectors(&mut t, 0, 0, 16).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_erase_sectors_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("permission denied"));

    let mut client = FirehoseClient::new();
    let result = client.erase_sectors(&mut t, 0, 0, 16).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_raw_xml_ack() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let resp = client.raw_xml(&mut t, r#"<custom />"#).await.unwrap();
    assert!(resp.is_ack());
}

#[tokio::test]
async fn test_raw_xml_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("bad command"));

    let mut client = FirehoseClient::new();
    let resp = client.raw_xml(&mut t, r#"<custom />"#).await.unwrap();
    assert!(resp.is_nak());
    assert_eq!(resp.error.as_deref(), Some("bad command"));
}

#[tokio::test]
async fn test_execute_command_sends_xml() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let cmd = qedl_firehose::FirehoseCommand::GetStorageInfo;
    let _ = client.execute_command(&mut t, &cmd).await;

    let written = t.written_data();
    assert!(!written.is_empty());
    let xml = String::from_utf8_lossy(written[0]);
    assert!(xml.contains("<getstorageinfo />"));
    assert!(xml.contains("<?xml"));
}

#[tokio::test]
async fn test_read_sectors_ack() {
    let mut t = MockTransport::new();
    t.set_timeout(Duration::from_millis(50));
    // First: ACK for the read command
    t.push_read_data(&ack_response());
    // Then: raw data (4 sectors * 512 = 2048 bytes)
    t.push_read_data(&vec![0x42u8; 2048]);
    // Then: final ACK
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let result = client.read_sectors(&mut t, 0, 0, 4).await;
    assert!(result.is_ok());
    let data = result.unwrap();
    assert_eq!(data.len(), 2048);
    assert!(data.iter().all(|&b| b == 0x42));
}

#[tokio::test]
async fn test_read_sectors_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("unauthorized"));

    let mut client = FirehoseClient::new();
    let result = client.read_sectors(&mut t, 0, 0, 4).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_program_sectors_ack() {
    let mut t = MockTransport::new();
    t.set_timeout(Duration::from_millis(50));
    // First: ACK for the program command
    t.push_read_data(&ack_response());
    // Then: final ACK after data write
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let data = vec![0xABu8; 2048];
    let result = client.program_sectors(&mut t, 0, 0, 4, &data).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_program_sectors_nak() {
    let mut t = MockTransport::new();
    t.push_read_data(&nak_response("write protected"));

    let mut client = FirehoseClient::new();
    let data = vec![0xABu8; 2048];
    let result = client.program_sectors(&mut t, 0, 0, 4, &data).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_configure_updates_client_fields() {
    let mut t = MockTransport::new();
    let response = r#"<?xml version="1.0" encoding="UTF-8" ?><data><response value="ACK" SECTOR_SIZE_IN_BYTES="4096" MaxPayloadSizeToTargetInBytes="2097152" total_sectors="8388608" /></data>"#;
    t.push_read_data(response.as_bytes());

    let mut client = FirehoseClient::new();
    client.configure(&mut t).await.unwrap();

    assert_eq!(client.sector_size(), 4096);
    assert_eq!(client.max_payload_size(), 2097152);
    assert_eq!(client.total_sectors, 8388608);
}

#[tokio::test]
async fn test_get_storage_info() {
    let mut t = MockTransport::new();
    t.push_read_data(&ack_response());

    let mut client = FirehoseClient::new();
    let resp = client.get_storage_info(&mut t).await.unwrap();
    assert!(resp.is_ack());
}
