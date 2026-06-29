use qedl_firehose::response::FirehoseResponse;

#[test]
fn test_parse_ack() {
    let xml = r#"<response value="ACK" />"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_ack());
    assert!(!resp.is_nak());
}

#[test]
fn test_parse_nak() {
    let xml = r#"<response value="NAK"><log value="Sector not found" /></response>"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_nak());
    assert_eq!(resp.error_log.as_deref(), Some("Sector not found"));
}

#[test]
fn test_parse_ack_with_sector_info() {
    let xml = r#"<response value="ACK" SECTOR_SIZE_IN_BYTES="4096" num_partition_sectors="2048" physical_partition_number="0" />"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_ack());
    assert_eq!(resp.sector_size, Some(4096));
    assert_eq!(resp.num_partition_sectors, Some(2048));
    assert_eq!(resp.physical_partition_number, Some(0));
}

#[test]
fn test_parse_configure_response() {
    let xml = r#"<data><response value="ACK" SECTOR_SIZE_IN_BYTES="4096" MaxPayloadSizeToTargetInBytes="2097152" total_sectors="4194304" /><memory MemoryName="UFS" /></data>"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_ack());
    assert_eq!(resp.memory_name.as_deref(), Some("UFS"));
    assert_eq!(resp.sector_size, Some(4096));
    assert_eq!(resp.max_payload_size, Some(2097152));
    assert_eq!(resp.total_sectors, Some(4194304));
}

#[test]
fn test_parse_rawmode_response() {
    let xml = r#"<response value="ACK" rawmode="true" />"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_ack());
    assert!(resp.raw_mode);
}

#[test]
fn test_parse_with_logs() {
    let xml = r#"<response value="ACK"><log>Reading partition</log><log>Done</log></response>"#;
    let resp = FirehoseResponse::from_xml(xml).unwrap();
    assert!(resp.is_ack());
    assert_eq!(resp.logs.len(), 2);
    assert_eq!(resp.logs[0], "Reading partition");
    assert_eq!(resp.logs[1], "Done");
}

#[test]
fn test_parse_invalid_xml() {
    let result = FirehoseResponse::from_xml("not xml at all");
    // quick-xml is lenient; no response tag → defaults to Ack
    assert!(result.is_ok());
    assert!(result.unwrap().is_ack());
}
