//! Firehose XML 命令定义
//!
//! 构建发送给设备的 XML 命令。Firehose 层只负责 XML 构建和 ACK 解析，
//! 不理解命令语义，保持最大兼容性。

#[derive(Debug, Clone)]
pub enum FirehoseCommand {
    Configure {
        memory_name: String,
        target_name: String,
        skip_storage_init: bool,
        zlp_aware_host: bool,
        max_payload_size: u32,
    },
    Read {
        sector_size: u32,
        num_sectors: u64,
        physical_partition: u8,
        start_sector: u64,
    },
    /// 写入扇区（program 命令，后续跟 binary payload）
    Program {
        sector_size: u32,
        num_sectors: u64,
        physical_partition: u8,
        start_sector: u64,
        filename: Option<String>,
    },
    Erase {
        sector_size: u32,
        num_sectors: u64,
        physical_partition: u8,
        start_sector: u64,
    },
    GetStorageInfo,
    /// Get SHA256 digest of partition sectors from device
    GetSha256Digest {
        sector_size: u32,
        num_sectors: u64,
        physical_partition: u8,
        start_sector: u64,
    },
    /// Read memory at physical address
    Peek {
        address: u64,
        size: u32,
    },
    /// Write memory at physical address
    Poke {
        address: u64,
        data: Vec<u8>,
    },
    Power {
        value: String, // "reset", "off", etc.
    },
    /// 原始 XML 透传（兜底）
    RawXml(String),
}

impl FirehoseCommand {
    /// Semantic command name for TRACE logging.
    /// Returns a human-readable summary like "configure", "read(lun=0, start=1, count=1)".
    pub fn name(&self) -> String {
        match self {
            Self::Configure { .. } => "configure".to_string(),
            Self::Read {
                physical_partition,
                start_sector,
                num_sectors,
                ..
            } => {
                format!(
                    "read(lun={}, start={}, count={})",
                    physical_partition, start_sector, num_sectors
                )
            }
            Self::Program {
                physical_partition,
                start_sector,
                num_sectors,
                ..
            } => {
                format!(
                    "program(lun={}, start={}, count={})",
                    physical_partition, start_sector, num_sectors
                )
            }
            Self::Erase {
                physical_partition,
                start_sector,
                num_sectors,
                ..
            } => {
                format!(
                    "erase(lun={}, start={}, count={})",
                    physical_partition, start_sector, num_sectors
                )
            }
            Self::GetStorageInfo => "getstorageinfo".to_string(),
            Self::GetSha256Digest {
                physical_partition,
                start_sector,
                num_sectors,
                ..
            } => {
                format!(
                    "getsha256digest(lun={}, start={}, count={})",
                    physical_partition, start_sector, num_sectors
                )
            }
            Self::Peek { address, size } => format!("peek(addr=0x{:X}, size={})", address, size),
            Self::Poke { address, data } => format!("poke(addr=0x{:X}, len={})", address, data.len()),
            Self::Power { value } => format!("power({})", value),
            Self::RawXml(_) => "raw-xml".to_string(),
        }
    }

    pub fn to_xml(&self) -> String {
        match self {
            Self::Configure {
                memory_name,
                target_name: _,
                skip_storage_init: _,
                zlp_aware_host: _,
                max_payload_size,
            } => {
                // Minimal QFIL-compatible configure: many loaders reject extra attributes
                format!(
                    r#"<configure MemoryName="{}" Verbose="0" MaxPayloadSizeToTargetInBytes="{}" />"#,
                    memory_name, max_payload_size
                )
            }
            Self::Read {
                sector_size,
                num_sectors,
                physical_partition,
                start_sector,
            } => {
                format!(
                    r#"<read SECTOR_SIZE_IN_BYTES="{}" num_partition_sectors="{}" physical_partition_number="{}" start_sector="{}" />"#,
                    sector_size, num_sectors, physical_partition, start_sector
                )
            }
            Self::Program {
                sector_size,
                num_sectors,
                physical_partition,
                start_sector,
                filename,
            } => {
                let mut xml = format!(
                    r#"<program SECTOR_SIZE_IN_BYTES="{}" num_partition_sectors="{}" physical_partition_number="{}" start_sector="{}""#,
                    sector_size, num_sectors, physical_partition, start_sector
                );
                if let Some(f) = filename {
                    xml.push_str(&format!(r#" filename="{}""#, f));
                }
                xml.push_str(" />");
                xml
            }
            Self::Erase {
                sector_size,
                num_sectors,
                physical_partition,
                start_sector,
            } => {
                format!(
                    r#"<erase SECTOR_SIZE_IN_BYTES="{}" num_partition_sectors="{}" physical_partition_number="{}" start_sector="{}" />"#,
                    sector_size, num_sectors, physical_partition, start_sector
                )
            }
            Self::GetStorageInfo => "<getstorageinfo />".to_string(),
            Self::GetSha256Digest {
                sector_size,
                num_sectors,
                physical_partition,
                start_sector,
            } => {
                format!(
                    r#"<getsha256digest SECTOR_SIZE_IN_BYTES="{}" num_partition_sectors="{}" physical_partition_number="{}" start_sector="{}" />"#,
                    sector_size, num_sectors, physical_partition, start_sector
                )
            }
            Self::Peek { address, size } => {
                format!(r#"<peek address64="{:#x}" size_in_bytes="{}" />"#, address, size)
            }
            Self::Poke { address, data } => {
                // Format data as "0xAA 0xBB 0xCC" for the value attribute
                let hex_values: Vec<String> = data.iter().map(|b| format!("0x{:02X}", b)).collect();
                format!(
                    r#"<poke address64="{:#x}" size_in_bytes="{}" value64="{}" />"#,
                    address,
                    data.len(),
                    hex_values.join(" ")
                )
            }
            Self::Power { value } => {
                format!(r#"<power value="{}" />"#, value)
            }
            Self::RawXml(xml) => xml.clone(),
        }
    }
}
