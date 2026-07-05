use crate::protocol::FirehoseInfo;
use crate::types::{DeviceCapabilities, DeviceInfo};

/// Session holds all device state after configuration.
///
/// Created after `configure()` completes successfully.
#[derive(Debug, Clone)]
pub struct Session {
    pub info: DeviceInfo,
    pub capabilities: DeviceCapabilities,
    pub firehose: FirehoseInfo,
    /// Raw MSM hardware ID bytes (from Sahara exec_cmd MSM_HW_ID_READ)
    pub msm_hw_id: Option<Vec<u8>>,
    /// Chip serial number (from Sahara exec_cmd SERIAL_NUM_READ)
    pub serial_num: Option<u64>,
}

impl Session {
    pub fn new(info: DeviceInfo, capabilities: DeviceCapabilities, firehose: FirehoseInfo) -> Self {
        Self {
            info,
            capabilities,
            firehose,
            msm_hw_id: None,
            serial_num: None,
        }
    }

    pub fn with_msm_hw_id(mut self, id: Vec<u8>) -> Self {
        self.msm_hw_id = Some(id);
        self
    }

    pub fn with_serial_num(mut self, num: u64) -> Self {
        self.serial_num = Some(num);
        self
    }

    pub fn sector_size(&self) -> u32 {
        self.firehose.sector_size
    }

    pub fn max_payload_size(&self) -> u32 {
        self.firehose.max_payload_size
    }

    pub fn memory_type(&self) -> &str {
        &self.capabilities.memory_type
    }

    pub fn total_sectors(&self) -> u64 {
        self.capabilities.total_sectors
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.capabilities.total_size_bytes(self.firehose.sector_size)
    }

    pub fn total_size_human(&self) -> String {
        self.capabilities.total_size_human(self.firehose.sector_size)
    }
}
