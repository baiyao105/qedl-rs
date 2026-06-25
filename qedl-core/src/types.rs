#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub name: String,
    pub first_lba: u64,
    pub last_lba: u64,
    pub physical_partition: u8,
}

impl PartitionInfo {
    pub fn size_bytes(&self, sector_size: u32) -> u64 {
        (self.last_lba - self.first_lba + 1) * sector_size as u64
    }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Serial port name (e.g., "COM3", "/dev/ttyUSB0")
    pub port: String,
    pub serial: Option<String>,
    pub product: Option<String>,
    pub pid: u16,
    /// USB Vendor ID (always 0x05C6 for Qualcomm)
    pub vid: u16,
    pub description: Option<String>,
}

impl DeviceInfo {
    pub fn is_9008(&self) -> bool {
        self.vid == 0x05C6 && self.pid == 0x9008
    }

    pub fn is_90b8(&self) -> bool {
        self.vid == 0x05C6 && self.pid == 0x90B8
    }
}

impl std::fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.port)?;
        if let Some(ref serial) = self.serial {
            write!(f, " (serial: {})", serial)?;
        }
        if let Some(ref desc) = self.description {
            write!(f, " - {}", desc)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    /// Storage type (e.g., "eMMC", "UFS")
    pub memory_type: String,
    pub total_sectors: u64,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            memory_type: "eMMC".to_string(),
            total_sectors: 0,
        }
    }
}

impl DeviceCapabilities {
    /// Total device size in bytes (requires sector_size from FirehoseInfo)
    pub fn total_size_bytes(&self, sector_size: u32) -> u64 {
        self.total_sectors * sector_size as u64
    }

    pub fn total_size_human(&self, sector_size: u32) -> String {
        let bytes = self.total_size_bytes(sector_size);
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
        } else if bytes >= 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
        } else if bytes >= 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    }
}

pub trait ProgressReporter: Send + Sync {
    fn start(&self, total: u64, message: &str);
    fn update(&self, current: u64);
    fn finish(&self, message: &str);
}

pub struct NoopProgress;

impl ProgressReporter for NoopProgress {
    fn start(&self, _total: u64, _message: &str) {}
    fn update(&self, _current: u64) {}
    fn finish(&self, _message: &str) {}
}
