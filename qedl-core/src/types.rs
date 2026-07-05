use humansize::{DECIMAL, format_size};

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

/// Qualcomm USB device operating mode, determined from interface descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    /// Firehose/Sahara mode (bInterfaceClass=0xFF, SubClass=0xFF, Protocol=0xFF)
    Edl,
    /// DIAG mode (bInterfaceClass=0xFF, SubClass=0xFF, Protocol≠0xFF)
    Diag,
    /// Could not determine from USB descriptors; fall back to PID heuristic
    Unknown,
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
    /// Device mode determined from USB interface descriptors
    pub mode: DeviceMode,
}

/// Known Qualcomm DIAG mode PIDs (fallback when interface descriptor unavailable).
pub const DIAG_PIDS: &[u16] = &[0x90B8, 0x9091, 0x90E8];

impl DeviceInfo {
    pub fn is_9008(&self) -> bool {
        match self.mode {
            DeviceMode::Edl => true,
            DeviceMode::Unknown => self.vid == 0x05C6 && self.pid == 0x9008,
            _ => false,
        }
    }

    pub fn is_90b8(&self) -> bool {
        self.vid == 0x05C6 && self.pid == 0x90B8
    }

    /// Returns true if this device is in any Qualcomm DIAG mode.
    /// Uses interface descriptor when available, falls back to PID heuristic.
    pub fn is_diag(&self) -> bool {
        match self.mode {
            DeviceMode::Diag => true,
            DeviceMode::Unknown => self.vid == 0x05C6 && DIAG_PIDS.contains(&self.pid),
            _ => false,
        }
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
        format_size(bytes, DECIMAL)
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
