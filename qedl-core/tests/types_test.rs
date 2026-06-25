use qedl_core::{DeviceCapabilities, PartitionInfo};

#[tokio::test]
async fn test_partition_size() {
    let p = PartitionInfo {
        name: "boot".to_string(),
        first_lba: 0,
        last_lba: 1023,
        physical_partition: 0,
    };
    assert_eq!(p.size_bytes(512), 512 * 1024);
}

#[tokio::test]
async fn test_capabilities_total_size() {
    let caps = DeviceCapabilities {
        memory_type: "eMMC".to_string(),
        total_sectors: 1024 * 1024 * 2,
    };
    assert_eq!(caps.total_size_bytes(512), 1024 * 1024 * 2 * 512);
}

#[tokio::test]
async fn test_capabilities_human_size() {
    let caps = DeviceCapabilities {
        memory_type: "eMMC".to_string(),
        total_sectors: 1024 * 1024 * 2,
    };
    let human = caps.total_size_human(512);
    assert!(human.contains("GB"));
}

#[tokio::test]
async fn test_capabilities_default() {
    let caps = DeviceCapabilities::default();
    assert_eq!(caps.memory_type, "eMMC");
    assert_eq!(caps.total_sectors, 0);
}

#[tokio::test]
async fn test_device_info_display() {
    let info = qedl_core::DeviceInfo {
        port: "COM3".to_string(),
        serial: None,
        product: None,
        pid: 0x9008,
        vid: 0x05C6,
        description: None,
    };
    assert_eq!(format!("{}", info), "COM3");
}
