#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SaharaCommand {
    Hello = 0x01,
    HelloResponse = 0x02,
    ReadData = 0x03,
    EndTransfer = 0x04,
    DoneRequest = 0x05,
    DoneResponse = 0x06,
    ResetRequest = 0x07,
    ResetResponse = 0x08,
    MemoryDebug = 0x09,
    MemoryRead = 0x0A,
    CmdReady = 0x0B,
    SwitchMode = 0x0C,
    ExecuteRequest = 0x0D,
    ExecuteResponse = 0x0E,
    ExecuteData = 0x0F,
    MemoryDebug64 = 0x10,
    MemoryRead64 = 0x11,
    ReadData64 = 0x12,
}

impl SaharaCommand {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0x01 => Some(Self::Hello),
            0x02 => Some(Self::HelloResponse),
            0x03 => Some(Self::ReadData),
            0x04 => Some(Self::EndTransfer),
            0x05 => Some(Self::DoneRequest),
            0x06 => Some(Self::DoneResponse),
            0x07 => Some(Self::ResetRequest),
            0x08 => Some(Self::ResetResponse),
            0x09 => Some(Self::MemoryDebug),
            0x0A => Some(Self::MemoryRead),
            0x0B => Some(Self::CmdReady),
            0x0C => Some(Self::SwitchMode),
            0x0D => Some(Self::ExecuteRequest),
            0x0E => Some(Self::ExecuteResponse),
            0x0F => Some(Self::ExecuteData),
            0x10 => Some(Self::MemoryDebug64),
            0x11 => Some(Self::MemoryRead64),
            0x12 => Some(Self::ReadData64),
            _ => None,
        }
    }
}

pub const HELLO_PACKET_SIZE: usize = 48;
pub const PROTOCOL_VERSION: u32 = 2;
pub const PROTOCOL_VERSION_MIN: u32 = 2;
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Sahara exec command IDs
pub mod exec_cmd {
    pub const SERIAL_NUM_READ: u32 = 0x01;
    pub const MSM_HW_ID_READ: u32 = 0x02;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SaharaMode {
    ImageTransfer = 0x00,
    ImageTransferComplete = 0x01,
    MemoryDebug = 0x02,
    Command = 0x03,
}
