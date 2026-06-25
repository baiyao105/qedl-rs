use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use qedl_core::{PartitionInfo, ProgressReporter};

#[derive(Debug, Clone)]
pub struct XmlResponse {
    pub is_ack: bool,
    pub error_log: Option<String>,
}

/// Abstraction over device I/O for job execution.
///
/// Jobs depend on this trait instead of concrete FirehoseClient/Transport/PartitionMap,
/// enabling testability and decoupling from transport-layer details.
#[async_trait]
pub trait JobContext: Send + Sync {
    async fn read_sectors(&mut self, physical_partition: u8, start_sector: u64, num_sectors: u64) -> Result<Bytes>;

    async fn write_sectors(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> Result<()>;

    async fn erase_sectors(&mut self, physical_partition: u8, start_sector: u64, num_sectors: u64) -> Result<()> {
        let sector_size = self.sector_size() as u64;
        let max_payload = self.max_payload_size() as u64;
        let sectors_per_chunk = (max_payload / sector_size).max(1);
        let chunk_bytes = (sectors_per_chunk * sector_size) as usize;

        let erase_buf = vec![0u8; chunk_bytes];
        let mut remaining = num_sectors;
        let mut sector = start_sector;

        while remaining > 0 {
            let chunk = remaining.min(sectors_per_chunk);
            let write_bytes = (chunk * sector_size) as usize;
            self.write_sectors(physical_partition, sector, chunk, &erase_buf[..write_bytes])
                .await?;
            sector += chunk;
            remaining -= chunk;
        }
        Ok(())
    }

    fn sector_size(&self) -> u32;

    fn max_payload_size(&self) -> u32;

    fn storage_name(&self) -> &str;

    fn total_sectors(&self) -> u64;

    fn find_partition(&self, name: &str) -> Option<&PartitionInfo>;

    fn all_partitions(&self) -> Vec<&PartitionInfo>;

    async fn reboot(&mut self) -> Result<()>;

    async fn raw_xml(&mut self, xml: &str) -> Result<XmlResponse>;

    async fn refresh_storage_info(&mut self) -> Result<Vec<String>>;

    fn progress(&self) -> &dyn ProgressReporter;
}
