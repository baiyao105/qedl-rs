//! Firehose XML command engine.

pub mod client;
pub mod command;
pub mod error;
pub mod response;

pub use client::FirehoseClient;
pub use command::FirehoseCommand;
pub use error::FirehoseError;
pub use response::FirehoseResponse;

use async_trait::async_trait;
use bytes::Bytes;
use qedl_transport::Transport;

/// Trait abstracting Firehose protocol operations.
/// Enables mock testing and alternative protocol implementations.
#[async_trait]
pub trait FirehoseProtocol: Send + Sync {
    async fn configure(&mut self, transport: &mut dyn Transport) -> error::Result<()>;
    async fn execute_command(
        &mut self,
        transport: &mut dyn Transport,
        command: &FirehoseCommand,
    ) -> error::Result<FirehoseResponse>;
    async fn read_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> error::Result<Bytes>;
    async fn program_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
        data: &[u8],
    ) -> error::Result<()>;
    async fn erase_sectors(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> error::Result<()>;
    async fn get_storage_info(&mut self, transport: &mut dyn Transport) -> error::Result<FirehoseResponse>;
    async fn get_sha256_digest(
        &mut self,
        transport: &mut dyn Transport,
        physical_partition: u8,
        start_sector: u64,
        num_sectors: u64,
    ) -> error::Result<String>;
    async fn peek(&mut self, transport: &mut dyn Transport, address: u64, size: u32) -> error::Result<Vec<u8>>;
    async fn poke(&mut self, transport: &mut dyn Transport, address: u64, data: &[u8]) -> error::Result<()>;
    async fn reboot(&mut self, transport: &mut dyn Transport) -> error::Result<()>;
    async fn raw_xml(&mut self, transport: &mut dyn Transport, xml: &str) -> error::Result<FirehoseResponse>;
    async fn drain_initial_messages(&mut self, transport: &mut dyn Transport) -> error::Result<()>;

    fn sector_size(&self) -> u32;
    fn max_payload_size(&self) -> u32;
    fn is_initialized(&self) -> bool;
    fn memory_name(&self) -> &str;
    fn target_name(&self) -> &str;
    fn version(&self) -> Option<&str>;
    fn max_payload_size_from_target(&self) -> Option<u32>;
    fn max_payload_size_to_target_supported(&self) -> Option<u32>;
    fn max_xml_size(&self) -> Option<u32>;
    fn total_sectors(&self) -> u64;
    fn update_from_storage_info(&mut self, sector_size: Option<u32>, total_sectors: Option<u64>);
    fn set_memory_name(&mut self, name: String);
}
