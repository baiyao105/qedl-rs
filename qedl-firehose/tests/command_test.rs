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
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_read_xml() {
    let cmd = FirehoseCommand::Read {
        sector_size: 4096,
        num_sectors: 128,
        physical_partition: 0,
        start_sector: 64,
    };
    insta::assert_snapshot!(cmd.to_xml());
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
    insta::assert_snapshot!(cmd.to_xml());
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
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_erase_xml() {
    let cmd = FirehoseCommand::Erase {
        sector_size: 512,
        num_sectors: 16,
        physical_partition: 0,
        start_sector: 100,
    };
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_getstorageinfo_xml() {
    let cmd = FirehoseCommand::GetStorageInfo;
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_power_reset_xml() {
    let cmd = FirehoseCommand::Power {
        value: "reset".to_string(),
    };
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_power_off_xml() {
    let cmd = FirehoseCommand::Power {
        value: "off".to_string(),
    };
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_raw_xml_passthrough() {
    let xml_str = r#"<custom foo="bar" />"#;
    let cmd = FirehoseCommand::RawXml(xml_str.to_string());
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_peek_xml() {
    let cmd = FirehoseCommand::Peek {
        address: 0x08071320,
        size: 4096,
    };
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_peek_name() {
    let cmd = FirehoseCommand::Peek {
        address: 0x08071320,
        size: 4096,
    };
    assert_eq!(cmd.name(), "peek(addr=0x8071320, size=4096)");
}

#[test]
fn test_poke_xml() {
    let cmd = FirehoseCommand::Poke {
        address: 0x08071320,
        data: vec![0xAA, 0xBB, 0xCC, 0xDD],
    };
    insta::assert_snapshot!(cmd.to_xml());
}

#[test]
fn test_poke_name() {
    let cmd = FirehoseCommand::Poke {
        address: 0x08071320,
        data: vec![0xAA, 0xBB, 0xCC, 0xDD],
    };
    assert_eq!(cmd.name(), "poke(addr=0x8071320, len=4)");
}
