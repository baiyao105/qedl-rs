use crate::protocol::FirehoseInfo;
use crate::types::{DeviceCapabilities, DeviceInfo};

/// Session holds all device state after configuration.
///
/// Created after `configure()` completes successfully.
#[derive(Debug, Clone)]
pub struct Session {
    pub(crate) info: DeviceInfo,
    pub(crate) capabilities: DeviceCapabilities,
    pub(crate) firehose: FirehoseInfo,
    /// Raw MSM hardware ID bytes (from Sahara exec_cmd MSM_HW_ID_READ)
    pub(crate) msm_hw_id: Option<Vec<u8>>,
    /// Chip serial number (from Sahara exec_cmd SERIAL_NUM_READ)
    pub(crate) serial_num: Option<u64>,
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

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn capabilities(&self) -> &DeviceCapabilities {
        &self.capabilities
    }

    pub fn firehose_info(&self) -> &FirehoseInfo {
        &self.firehose
    }

    pub fn msm_hw_id(&self) -> Option<&[u8]> {
        self.msm_hw_id.as_deref()
    }

    pub fn serial_num(&self) -> Option<u64> {
        self.serial_num
    }

    pub fn set_msm_hw_id(&mut self, id: Option<Vec<u8>>) {
        self.msm_hw_id = id;
    }

    pub fn set_serial_num(&mut self, num: Option<u64>) {
        self.serial_num = num;
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

    /// Update session state from a configured FirehoseClient.
    /// Centralizes the field-copy logic that was previously duplicated in executor.
    #[allow(clippy::too_many_arguments)]
    pub fn update_from_firehose(
        &mut self,
        sector_size: u32,
        max_payload_size: u32,
        max_payload_size_from_target: Option<u32>,
        max_payload_size_to_target_supported: Option<u32>,
        max_xml_size: Option<u32>,
        target_name: &str,
        version: Option<&str>,
        memory_name: &str,
        total_sectors: u64,
    ) {
        self.firehose.sector_size = sector_size;
        self.firehose.max_payload_size = max_payload_size;
        self.firehose.max_payload_size_from_target = max_payload_size_from_target;
        self.firehose.max_payload_size_to_target_supported = max_payload_size_to_target_supported;
        self.firehose.max_xml_size = max_xml_size;
        self.firehose.target_name = Some(target_name.to_string());
        self.firehose.version = version.map(|s| s.to_string());
        self.capabilities.memory_type = memory_name.to_string();
        self.capabilities.total_sectors = total_sectors;
    }
}
