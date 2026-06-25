use crate::error::{Result, StorageError};

const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = 0xEDB88320 ^ (crc >> 1);
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc = crc32_byte(crc, byte);
    }
    crc ^ 0xFFFFFFFF
}

fn crc32_byte(crc: u32, byte: u8) -> u32 {
    CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8)
}

#[derive(Debug, Clone)]
pub struct GptHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_start_lba: u64,
    pub num_partition_entries: u32,
    pub partition_entry_size: u32,
    pub partition_entries_crc32: u32,
}

impl GptHeader {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 128 {
            return Err(StorageError::InvalidSignature {
                got: format!("too short: {} bytes", data.len()),
            });
        }

        let signature: [u8; 8] = data[0..8]
            .try_into()
            .map_err(|_| StorageError::InvalidEntry { index: 0 })?;
        if signature != *GPT_SIGNATURE {
            let got_str = String::from_utf8_lossy(&signature).to_string();
            if got_str.starts_with("<?xml") {
                return Err(StorageError::InvalidSignature {
                    got: format!(
                        "Firehose XML response instead of GPT data: {}",
                        String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 64)])
                    ),
                });
            }
            return Err(StorageError::InvalidSignature { got: got_str });
        }

        Ok(Self {
            signature,
            revision: u32::from_le_bytes(
                data[8..12]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            header_size: u32::from_le_bytes(
                data[12..16]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            header_crc32: u32::from_le_bytes(
                data[16..20]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            current_lba: u64::from_le_bytes(
                data[24..32]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            backup_lba: u64::from_le_bytes(
                data[32..40]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            first_usable_lba: u64::from_le_bytes(
                data[40..48]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            last_usable_lba: u64::from_le_bytes(
                data[48..56]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            disk_guid: data[56..72]
                .try_into()
                .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            partition_entry_start_lba: u64::from_le_bytes(
                data[72..80]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            num_partition_entries: u32::from_le_bytes(
                data[80..84]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            partition_entry_size: u32::from_le_bytes(
                data[84..88]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
            partition_entries_crc32: u32::from_le_bytes(
                data[88..92]
                    .try_into()
                    .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
            ),
        })
    }

    pub fn verify_header_crc(&self, raw_header: &[u8]) -> bool {
        let hdr_size = self.header_size as usize;
        if hdr_size < 92 || raw_header.len() < hdr_size {
            return false;
        }
        let mut crc = 0xFFFFFFFFu32;
        for (i, &byte) in raw_header[..hdr_size].iter().enumerate() {
            if (16..20).contains(&i) {
                crc = crc32_byte(crc, 0);
            } else {
                crc = crc32_byte(crc, byte);
            }
        }
        (crc ^ 0xFFFFFFFF) == self.header_crc32
    }
}

#[derive(Debug, Clone)]
pub struct GptEntry {
    pub name: String,
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub attributes: u64,
    pub physical_partition: u8,
}

impl GptEntry {
    pub fn from_bytes(data: &[u8], physical_partition: u8) -> Result<Self> {
        if data.len() < 128 {
            return Err(StorageError::InvalidEntry { index: 0 });
        }

        let type_guid: [u8; 16] = data[0..16]
            .try_into()
            .map_err(|_| StorageError::InvalidEntry { index: 0 })?;
        let unique_guid: [u8; 16] = data[16..32]
            .try_into()
            .map_err(|_| StorageError::InvalidEntry { index: 0 })?;
        let first_lba = u64::from_le_bytes(
            data[32..40]
                .try_into()
                .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
        );
        let last_lba = u64::from_le_bytes(
            data[40..48]
                .try_into()
                .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
        );
        let attributes = u64::from_le_bytes(
            data[48..56]
                .try_into()
                .map_err(|_| StorageError::InvalidEntry { index: 0 })?,
        );

        let name_bytes = &data[56..128];
        let name = decode_utf16le(name_bytes);

        Ok(Self {
            name,
            type_guid,
            unique_guid,
            first_lba,
            last_lba,
            attributes,
            physical_partition,
        })
    }

    pub fn size_bytes(&self, sector_size: u32) -> u64 {
        (self.last_lba - self.first_lba + 1) * sector_size as u64
    }

    pub fn size_kb(&self, sector_size: u32) -> u64 {
        self.size_bytes(sector_size) / 1024
    }

    pub fn is_empty(&self) -> bool {
        self.type_guid == [0u8; 16]
    }
}

#[derive(Debug, Clone)]
pub struct GptTable {
    pub primary_valid: bool,
    pub backup_valid: bool,
    pub header: Option<GptHeader>,
    pub entries: Vec<GptEntry>,
    pub physical_partition: u8,
    pub sector_size: u32,
}

impl GptTable {
    pub fn new() -> Self {
        Self {
            primary_valid: false,
            backup_valid: false,
            header: None,
            entries: Vec::new(),
            physical_partition: 0,
            sector_size: 512,
        }
    }

    pub fn parse(lba1_data: &[u8], entries_data: &[u8], physical_partition: u8, sector_size: u32) -> Result<Self> {
        let header = GptHeader::from_bytes(lba1_data)?;

        if !header.verify_header_crc(lba1_data) {
            return Err(StorageError::CrcMismatch {
                primary: true,
                expected: header.header_crc32,
                actual: crc32(lba1_data),
            });
        }

        let mut entries = Vec::new();
        let entry_size = header.partition_entry_size as usize;
        let num_entries = header.num_partition_entries as usize;

        for i in 0..num_entries {
            let offset = i * entry_size;
            if offset + entry_size > entries_data.len() {
                break;
            }
            let entry = GptEntry::from_bytes(&entries_data[offset..offset + entry_size], physical_partition)?;
            if !entry.is_empty() {
                entries.push(entry);
            }
        }

        let expected_len = num_entries * entry_size;
        if entries_data.len() >= expected_len {
            let partition_entries_crc = crc32(&entries_data[..expected_len]);
            if partition_entries_crc != header.partition_entries_crc32 {
                tracing::warn!(
                    "GPT partition entries CRC mismatch: expected {:#010x}, computed {:#010x}",
                    header.partition_entries_crc32,
                    partition_entries_crc
                );
            }
        }

        Ok(Self {
            primary_valid: true,
            backup_valid: false,
            header: Some(header),
            entries,
            physical_partition,
            sector_size,
        })
    }

    pub fn find_partition(&self, name: &str) -> Option<&GptEntry> {
        let name_lower = name.trim().to_lowercase();
        self.entries.iter().find(|e| {
            let entry_name = e.name.trim().trim_matches('\0').trim().to_lowercase();
            entry_name == name_lower
        })
    }

    pub fn partition_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|e| e.name.trim().trim_end_matches('\0'))
            .collect()
    }
}

impl Default for GptTable {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_utf16le(data: &[u8]) -> String {
    let u16s: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf16_lossy(&u16s)
}
