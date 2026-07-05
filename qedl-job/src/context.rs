use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use qedl_core::{PartitionInfo, ProgressReporter, Session};

#[derive(Debug, Clone)]
pub struct XmlResponse {
    pub is_ack: bool,
    pub error: Option<String>,
}

/// Shared zero buffer for write-zero erase operations.
/// Using a static buffer avoids repeated allocations and improves cache locality.
/// Size is 512KB which is a common max payload for Firehose.
static ZERO_BUF: [u8; 512 * 1024] = [0u8; 512 * 1024];

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
        // Use the larger of max_payload or the static zero buffer size for fewer round-trips
        let chunk_bytes = (max_payload as usize).max(ZERO_BUF.len());
        let sectors_per_chunk = (chunk_bytes as u64 / sector_size).max(1);

        let mut remaining = num_sectors;
        let mut sector = start_sector;

        while remaining > 0 {
            let chunk = remaining.min(sectors_per_chunk);
            let write_bytes = (chunk * sector_size) as usize;
            // Use slices from the static zero buffer
            self.write_sectors(physical_partition, sector, chunk, &ZERO_BUF[..write_bytes])
                .await?;
            sector += chunk;
            remaining -= chunk;
        }
        Ok(())
    }

    /// Native erase using Firehose erase command (faster than write-zero for supported devices).
    /// NOTE: Some Firehose implementations have bugs with the erase command.
    /// Use WriteZero method as default for safety.
    async fn erase_sectors_native(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<()> {
        self.erase_sectors(physical_partition, start_sector, num_sectors).await
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

    /// Get SHA256 digest of partition sectors from device.
    async fn get_sha256_digest(
        &mut self,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> Result<String>;

    fn progress(&self) -> &dyn ProgressReporter;

    fn session(&self) -> Option<&Session>;
}
