use qedl_storage::gpt::{GptEntry, GptHeader, GptTable};

fn make_valid_header_bytes() -> Vec<u8> {
    let mut data = vec![0u8; 512];
    data[0..8].copy_from_slice(b"EFI PART");
    data[8..12].copy_from_slice(&1u32.to_le_bytes());
    data[12..16].copy_from_slice(&92u32.to_le_bytes());
    data[24..32].copy_from_slice(&1u64.to_le_bytes());
    data[32..40].copy_from_slice(&0xFFFFFFFFu64.to_le_bytes());
    data[40..48].copy_from_slice(&34u64.to_le_bytes());
    data[48..56].copy_from_slice(&0xFFFFFFu64.to_le_bytes());
    data[72..80].copy_from_slice(&2u64.to_le_bytes());
    data[80..84].copy_from_slice(&128u32.to_le_bytes());
    data[84..88].copy_from_slice(&128u32.to_le_bytes());
    data
}

fn make_entry_bytes(name: &str, first_lba: u64, last_lba: u64) -> Vec<u8> {
    let mut data = vec![0u8; 128];
    data[0] = 0x01;
    data[16] = 0x02;
    data[32..40].copy_from_slice(&first_lba.to_le_bytes());
    data[40..48].copy_from_slice(&last_lba.to_le_bytes());
    for (i, ch) in name.encode_utf16().enumerate() {
        let offset = 56 + i * 2;
        if offset + 2 <= 128 {
            data[offset..offset + 2].copy_from_slice(&ch.to_le_bytes());
        }
    }
    data
}

fn make_table_with_names(names: &[&str]) -> GptTable {
    let mut table = GptTable::new();
    table.primary_valid = true;
    table.physical_partition = 0;
    table.sector_size = 512;
    for (i, name) in names.iter().enumerate() {
        table.entries.push(GptEntry {
            name: name.to_string(),
            type_guid: [(i + 1) as u8; 16],
            unique_guid: [(i + 10) as u8; 16],
            first_lba: (i as u64 + 1) * 1024,
            last_lba: (i as u64 + 2) * 1023,
            attributes: 0,
            physical_partition: 0,
        });
    }
    table
}

#[test]
fn test_parse_valid_header() {
    let data = make_valid_header_bytes();
    let header = GptHeader::from_bytes(&data).unwrap();
    assert_eq!(&header.signature, b"EFI PART");
    assert_eq!(header.revision, 1);
    assert_eq!(header.header_size, 92);
    assert_eq!(header.current_lba, 1);
    assert_eq!(header.num_partition_entries, 128);
    assert_eq!(header.partition_entry_size, 128);
}

#[test]
fn test_parse_header_too_short() {
    let result = GptHeader::from_bytes(&[0u8; 64]);
    assert!(result.is_err());
}

#[test]
fn test_parse_header_wrong_signature() {
    let mut data = vec![0u8; 128];
    data[0..8].copy_from_slice(b"NOT GPT!");
    let result = GptHeader::from_bytes(&data);
    assert!(result.is_err());
}

#[test]
fn test_parse_header_xml_signature() {
    let mut data = vec![0u8; 128];
    data[0..8].copy_from_slice(b"<?xml ve");
    let result = GptHeader::from_bytes(&data);
    assert!(result.is_err());
}

#[test]
fn test_parse_valid_entry() {
    let data = make_entry_bytes("boot", 64, 1023);
    let entry = GptEntry::from_bytes(&data, 0).unwrap();
    assert_eq!(entry.name, "boot");
    assert_eq!(entry.first_lba, 64);
    assert_eq!(entry.last_lba, 1023);
    assert_eq!(entry.physical_partition, 0);
}

#[test]
fn test_entry_size_bytes() {
    let entry = GptEntry {
        name: "test".to_string(),
        type_guid: [1; 16],
        unique_guid: [2; 16],
        first_lba: 0,
        last_lba: 1023,
        attributes: 0,
        physical_partition: 0,
    };
    assert_eq!(entry.size_bytes(512), 1024 * 512);
    assert_eq!(entry.size_bytes(4096), 1024 * 4096);
}

#[test]
fn test_entry_is_empty() {
    let empty = GptEntry {
        name: "".to_string(),
        type_guid: [0; 16],
        unique_guid: [0; 16],
        first_lba: 0,
        last_lba: 0,
        attributes: 0,
        physical_partition: 0,
    };
    assert!(empty.is_empty());

    let non_empty = GptEntry {
        name: "boot".to_string(),
        type_guid: [1; 16],
        unique_guid: [0; 16],
        first_lba: 0,
        last_lba: 100,
        attributes: 0,
        physical_partition: 0,
    };
    assert!(!non_empty.is_empty());
}

#[test]
fn test_gpt_table_find_partition() {
    let table = make_table_with_names(&["boot", "system"]);
    assert!(table.find_partition("boot").is_some());
    assert!(table.find_partition("system").is_some());
    assert!(table.find_partition("nonexistent").is_none());
}

#[test]
fn test_gpt_table_find_partition_case_insensitive() {
    let table = make_table_with_names(&["Boot"]);
    assert!(table.find_partition("boot").is_some());
    assert!(table.find_partition("BOOT").is_some());
}

#[test]
fn test_gpt_table_partition_names() {
    let table = make_table_with_names(&["boot", "system"]);
    let names = table.partition_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"boot"));
    assert!(names.contains(&"system"));
}

#[test]
fn test_gpt_table_default() {
    let table = GptTable::default();
    assert!(!table.primary_valid);
    assert!(!table.backup_valid);
    assert!(table.header.is_none());
    assert!(table.entries.is_empty());
}

#[test]
fn test_entry_name_utf16_roundtrip() {
    let data = make_entry_bytes("system", 100, 200);
    let entry = GptEntry::from_bytes(&data, 1).unwrap();
    assert_eq!(entry.name, "system");
    assert_eq!(entry.first_lba, 100);
    assert_eq!(entry.last_lba, 200);
    assert_eq!(entry.physical_partition, 1);
}
