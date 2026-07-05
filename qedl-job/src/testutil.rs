use crate::context::{JobContext, XmlResponse};
use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use qedl_core::{NoopProgress, PartitionInfo, ProgressReporter, Session};
use std::collections::HashMap;

pub struct MockJobContext {
    pub sector_size: u32,
    pub max_payload_size: u32,
    pub storage_name: String,
    pub total_sectors: u64,
    pub partitions: HashMap<String, PartitionInfo>,
    pub read_data: Vec<Bytes>,
    pub read_index: usize,
    pub write_log: Vec<(u8, u64, u64, Vec<u8>)>,
    pub erase_log: Vec<(u8, u64, u64)>,
    pub reboot_called: bool,
    pub xml_responses: Vec<XmlResponse>,
    pub xml_index: usize,
    pub storage_info_response: Option<std::result::Result<Vec<String>, String>>,
}

impl MockJobContext {
    pub fn simple() -> Self {
        let mut partitions = HashMap::new();
        partitions.insert(
            "boot".to_string(),
            PartitionInfo {
                name: "boot".to_string(),
                first_lba: 0,
                last_lba: 1023,
                physical_partition: 0,
            },
        );
        Self {
            sector_size: 512,
            max_payload_size: 1024 * 1024,
            storage_name: "eMMC".to_string(),
            total_sectors: 1024 * 1024,
            partitions,
            read_data: vec![],
            read_index: 0,
            write_log: vec![],
            erase_log: vec![],
            reboot_called: false,
            xml_responses: vec![],
            xml_index: 0,
            storage_info_response: None,
        }
    }

    pub fn push_read(&mut self, data: Bytes) {
        self.read_data.push(data);
    }

    pub fn push_xml_ack(&mut self) {
        self.xml_responses.push(XmlResponse {
            is_ack: true,
            error_log: None,
        });
    }

    pub fn push_xml_nak(&mut self, reason: &str) {
        self.xml_responses.push(XmlResponse {
            is_ack: false,
            error_log: Some(reason.to_string()),
        });
    }
}

#[async_trait]
impl JobContext for MockJobContext {
    async fn read_sectors(&mut self, _physical_partition: u8, _start_sector: u64, num_sectors: u64) -> Result<Bytes> {
        let bytes = (num_sectors * self.sector_size as u64) as usize;
        if self.read_index < self.read_data.len() {
            let data = self.read_data[self.read_index].clone();
            self.read_index += 1;
            Ok(data)
        } else {
            Ok(Bytes::from(vec![0xABu8; bytes]))
        }
    }

    async fn write_sectors(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> Result<()> {
        self.write_log
            .push((physical_partition, start_sector, num_sectors, data.to_vec()));
        Ok(())
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }

    fn max_payload_size(&self) -> u32 {
        self.max_payload_size
    }

    fn storage_name(&self) -> &str {
        &self.storage_name
    }

    fn total_sectors(&self) -> u64 {
        self.total_sectors
    }

    fn find_partition(&self, name: &str) -> Option<&PartitionInfo> {
        let name_lower = name.trim().to_lowercase();
        self.partitions
            .values()
            .find(|p| p.name.trim().to_lowercase() == name_lower)
    }

    fn all_partitions(&self) -> Vec<&PartitionInfo> {
        self.partitions.values().collect()
    }

    async fn reboot(&mut self) -> Result<()> {
        self.reboot_called = true;
        Ok(())
    }

    async fn raw_xml(&mut self, _xml: &str) -> Result<XmlResponse> {
        if self.xml_index < self.xml_responses.len() {
            let resp = self.xml_responses[self.xml_index].clone();
            self.xml_index += 1;
            Ok(resp)
        } else {
            Ok(XmlResponse {
                is_ack: true,
                error_log: None,
            })
        }
    }

    async fn refresh_storage_info(&mut self) -> Result<Vec<String>> {
        match &self.storage_info_response {
            Some(Ok(logs)) => Ok(logs.clone()),
            Some(Err(e)) => Err(crate::error::JobError::PreconditionFailed { reason: e.clone() }),
            None => Ok(vec![]),
        }
    }

    async fn erase_sectors(&mut self, physical_partition: u8, start_sector: u64, num_sectors: u64) -> Result<()> {
        self.erase_log.push((physical_partition, start_sector, num_sectors));
        Ok(())
    }

    async fn erase_sectors_native(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<()> {
        self.erase_log.push((physical_partition, start_sector, num_sectors));
        Ok(())
    }

    fn progress(&self) -> &dyn ProgressReporter {
        &NoopProgress
    }

    fn session(&self) -> Option<&Session> {
        None
    }
}
