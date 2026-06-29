use qedl_storage::PartitionMap;
use qedl_storage::gpt::{GptEntry, GptTable};

fn make_table(lun: u8, entry_names: &[&str]) -> GptTable {
    let mut table = GptTable::new();
    table.physical_partition = lun;
    table.primary_valid = true;

    for (i, name) in entry_names.iter().enumerate() {
        table.entries.push(GptEntry {
            name: name.to_string(),
            type_guid: [(i + 1) as u8; 16],
            unique_guid: [(i + 10) as u8; 16],
            first_lba: (i as u64 + 1) * 1024,
            last_lba: (i as u64 + 2) * 1023,
            attributes: 0,
            physical_partition: lun,
        });
    }
    table
}

#[test]
fn test_empty_partition_map() {
    let map = PartitionMap::new();
    assert_eq!(map.total_partitions(), 0);
    assert!(map.all_entries().is_empty());
    assert!(map.find_partition("boot").is_none());
}

#[test]
fn test_add_single_table() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["boot", "system"]));
    assert_eq!(map.total_partitions(), 2);
}

#[test]
fn test_find_partition_across_luns() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["boot"]));
    map.add_table(make_table(1, &["system"]));

    assert!(map.find_partition("boot").is_some());
    assert!(map.find_partition("system").is_some());
    assert!(map.find_partition("nonexistent").is_none());
}

#[test]
fn test_find_partition_o1_index() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["alpha", "beta", "gamma"]));

    let entry = map.find_partition("beta").unwrap();
    assert_eq!(entry.name, "beta");
    assert_eq!(entry.first_lba, 2048);
}

#[test]
fn test_all_entries() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["a", "b"]));
    map.add_table(make_table(1, &["c"]));

    let all = map.all_entries();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_entries_for_lun() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["a", "b"]));
    map.add_table(make_table(1, &["c", "d"]));

    let lun0 = map.entries_for_lun(0);
    assert_eq!(lun0.len(), 2);
    let lun1 = map.entries_for_lun(1);
    assert_eq!(lun1.len(), 2);
    let lun2 = map.entries_for_lun(2);
    assert!(lun2.is_empty());
}

#[test]
fn test_luns() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["a"]));
    map.add_table(make_table(1, &["b"]));
    map.add_table(make_table(0, &["c"]));

    let luns = map.luns();
    assert_eq!(luns, vec![0, 1, 0]);
}

#[test]
fn test_total_partitions() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["a", "b"]));
    map.add_table(make_table(1, &["c"]));
    assert_eq!(map.total_partitions(), 3);
}

#[test]
fn test_duplicate_name_first_wins() {
    let mut map = PartitionMap::new();
    map.add_table(make_table(0, &["boot"]));
    map.add_table(make_table(1, &["boot"]));

    let entry = map.find_partition("boot").unwrap();
    assert_eq!(entry.physical_partition, 0);
}
