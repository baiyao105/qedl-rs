use qedl_job::parser::RawProgram;

#[test]
fn test_parse_rawprogram() {
    let xml = r#"
    <?xml version="1.0" ?>
    <data>
        <program SECTOR_START="0" NUM_SECTORS="1024" file="boot.img" physical_partition_number="0" />
        <program SECTOR_START="1024" NUM_SECTORS="2048" file="system.img" physical_partition_number="0" sparse="true" />
    </data>
    "#;

    let program = RawProgram::parse_str(xml).unwrap();
    assert_eq!(program.entries.len(), 2);
    assert_eq!(program.entries[0].file, "boot.img");
    assert_eq!(program.entries[0].sector_start, 0);
    assert_eq!(program.entries[0].num_sectors, 1024);
    assert!(program.entries[1].sparse);
}

#[test]
fn test_validate_program() {
    let xml = r#"
    <?xml version="1.0" ?>
    <data>
        <program SECTOR_START="0" NUM_SECTORS="1024" file="boot.img" physical_partition_number="0" />
    </data>
    "#;

    let program = RawProgram::parse_str(xml).unwrap();
    assert!(program.validate().is_ok());
}

#[test]
fn test_validate_missing_file() {
    let xml = r#"
    <?xml version="1.0" ?>
    <data>
        <program SECTOR_START="0" NUM_SECTORS="1024" file="" physical_partition_number="0" />
    </data>
    "#;

    let program = RawProgram::parse_str(xml).unwrap();
    let result = program.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|e| e.contains("file")));
}

#[test]
fn test_validate_zero_sectors() {
    let xml = r#"
    <?xml version="1.0" ?>
    <data>
        <program SECTOR_START="0" NUM_SECTORS="0" file="boot.img" physical_partition_number="0" />
    </data>
    "#;

    let program = RawProgram::parse_str(xml).unwrap();
    let result = program.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|e| e.contains("NUM_SECTORS")));
}
