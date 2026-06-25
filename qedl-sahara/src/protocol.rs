//! Sahara 协议消息格式、命令码和状态码。

pub use qedl_core::{SaharaCommand, SaharaMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ImageId {
    OemSbl = 0x01,
    Amss = 0x05,
    Firehose = 0x07,
    Storage = 0x0A,
    Dsps = 0x0B,
    Apps = 0x0C,
    FirehoseV2 = 0x0D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SaharaStatus {
    Success = 0x00,
    InvalidCmd = 0x01,
    ProtocolMismatch = 0x02,
    InvalidTargetProtocol = 0x03,
    InvalidHostProtocol = 0x04,
    InvalidPacketSize = 0x05,
    UnexpectedImageId = 0x06,
    InvalidHeaderSize = 0x07,
    InvalidDataSize = 0x08,
    InvalidImageType = 0x09,
    InvalidTxLength = 0x0A,
    InvalidRxLength = 0x0B,
    GeneralTxRxError = 0x0C,
    ReadDataError = 0x0D,
}

#[derive(Debug, Clone)]
pub struct SaharaHello {
    pub command: SaharaCommand,
    pub packet_length: u32,
    pub version: u32,
    pub version_min: u32,
    pub max_command_length: u32,
    pub mode: SaharaMode,
    pub reserved: [u32; 6],
}

#[derive(Debug, Clone)]
pub struct SaharaHelloResponse {
    pub command: SaharaCommand,
    pub packet_length: u32,
    pub version: u32,
    pub version_min: u32,
    pub max_command_length: u32,
    pub mode: SaharaMode,
    pub reserved: [u32; 6],
}

#[derive(Debug, Clone)]
pub struct SaharaExecCmd {
    pub command: SaharaCommand,
    pub packet_length: u32,
    pub client_cmd: u32,
}

#[derive(Debug, Clone)]
pub struct SaharaExecRsp {
    pub command: SaharaCommand,
    pub packet_length: u32,
    pub client_cmd: u32,
    pub data_len: u32,
}

pub mod consts {
    pub use qedl_core::{HELLO_PACKET_SIZE, PROTOCOL_VERSION, PROTOCOL_VERSION_MIN};
    pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;
}
