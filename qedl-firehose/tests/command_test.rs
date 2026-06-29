use qedl_firehose::command::FirehoseCommand;

#[test]
fn test_configure_xml_minimal() {
    let cmd = FirehoseCommand::Configure {
        memory_name: "eMMC".to_string(),
        target_name: "unknown".to_string(),
        skip_storage_init: false,
        zlp_aware_host: true,
        max_payload_size: 1048576,
    };
    let xml = cmd.to_xml();
    assert!(xml.contains("MemoryName=\"eMMC\""));
    assert!(xml.contains("MaxPayloadSizeToTargetInBytes=\"1048576\""));
    assert!(xml.starts_with("<configure"));
    assert!(xml.ends_with("/>"));
}

#[test]
fn test_read_xml() {
    let cmd = FirehoseCommand::Read {
        sector_size: 4096,
        num_sectors: 128,
        physical_partition: 0,
        start_sector: 64,
    };
    let xml = cmd.to_xml();
    assert!(xml.contains("SECTOR_SIZE_IN_BYTES=\"4096\""));
    assert!(xml.contains("num_partition_sectors=\"128\""));
    assert!(xml.contains("physical_partition_number=\"0\""));
    assert!(xml.contains("start_sector=\"64\""));
}

#[test]
fn test_program_xml() {
    let cmd = FirehoseCommand::Program {
        sector_size: 512,
        num_sectors: 256,
        physical_partition: 1,
        start_sector: 0,
        filename: Some("boot.img".to_string()),
    };
    let xml = cmd.to_xml();
    assert!(xml.contains("filename=\"boot.img\""));
    assert!(xml.contains("physical_partition_number=\"1\""));
}

#[test]
fn test_program_xml_no_filename() {
    let cmd = FirehoseCommand::Program {
        sector_size: 512,
        num_sectors: 256,
        physical_partition: 0,
        start_sector: 0,
        filename: None,
    };
    let xml = cmd.to_xml();
    assert!(!xml.contains("filename="));
}

#[test]
fn test_erase_xml() {
    let cmd = FirehoseCommand::Erase {
        sector_size: 512,
        num_sectors: 16,
        physical_partition: 0,
        start_sector: 100,
    };
    let xml = cmd.to_xml();
    assert!(xml.contains("<erase"));
    assert!(xml.contains("num_partition_sectors=\"16\""));
}

#[test]
fn test_getstorageinfo_xml() {
    let cmd = FirehoseCommand::GetStorageInfo;
    let xml = cmd.to_xml();
    assert_eq!(xml, "<getstorageinfo />");
}

#[test]
fn test_power_reset_xml() {
    let cmd = FirehoseCommand::Power {
        value: "reset".to_string(),
    };
    let xml = cmd.to_xml();
    assert_eq!(xml, r#"<power value="reset" />"#);
}

#[test]
fn test_power_off_xml() {
    let cmd = FirehoseCommand::Power {
        value: "off".to_string(),
    };
    let xml = cmd.to_xml();
    assert_eq!(xml, r#"<power value="off" />"#);
}

#[test]
fn test_raw_xml_passthrough() {
    let xml_str = r#"<custom foo="bar" />"#;
    let cmd = FirehoseCommand::RawXml(xml_str.to_string());
    assert_eq!(cmd.to_xml(), xml_str);
}
