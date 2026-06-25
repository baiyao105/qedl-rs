use crate::error::{ImageError, Result};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PatchEntry {
    pub byte_offset: u32,
    pub filename: String,
    pub physical_partition: u8,
    pub size_in_bytes: u32,
    pub start_sector: u64,
    pub value: String,
}

impl PatchEntry {
    pub fn resolve_value(&self, vars: &HashMap<String, String>) -> Result<Vec<u8>> {
        let resolved = if self.value.starts_with("NUM_DISK_SECTORS") {
            vars.get("NUM_DISK_SECTORS")
                .ok_or_else(|| ImageError::ParseFailed {
                    path: PathBuf::from(&self.filename),
                    reason: "NUM_DISK_SECTORS not set".to_string(),
                })?
                .clone()
        } else if self.value.starts_with("LAST_PARTITION_END") {
            vars.get("LAST_PARTITION_END")
                .ok_or_else(|| ImageError::ParseFailed {
                    path: PathBuf::from(&self.filename),
                    reason: "LAST_PARTITION_END not set".to_string(),
                })?
                .clone()
        } else {
            self.value.clone()
        };

        let num: u64 = if resolved.starts_with("0x") || resolved.starts_with("0X") {
            u64::from_str_radix(&resolved[2..], 16)
        } else {
            resolved.parse()
        }
        .map_err(|_| ImageError::ParseFailed {
            path: PathBuf::from(&self.filename),
            reason: format!("invalid patch value: {}", resolved),
        })?;

        Ok(num.to_le_bytes()[..self.size_in_bytes as usize].to_vec())
    }
}

#[derive(Debug, Clone)]
pub struct PatchSet {
    pub entries: Vec<PatchEntry>,
    pub source_file: PathBuf,
}

use qedl_core::util::get_attr;

impl PatchSet {
    pub fn new(source_file: PathBuf) -> Self {
        Self {
            entries: Vec::new(),
            source_file,
        }
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| ImageError::ParseFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        Self::from_str(&content, path)
    }

    pub fn from_str(xml: &str, source: &Path) -> Result<Self> {
        let mut set = PatchSet::new(source.to_path_buf());
        let mut reader = Reader::from_str(xml);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag.to_lowercase() == "patch"
                        && let Some(entry) = parse_patch_entry(e)
                    {
                        set.entries.push(entry);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(ImageError::ParseFailed {
                        path: source.to_path_buf(),
                        reason: format!("XML parse error: {}", e),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(set)
    }
}

fn parse_patch_entry(e: &quick_xml::events::BytesStart) -> Option<PatchEntry> {
    Some(PatchEntry {
        byte_offset: get_attr(e, "byte_offset")?.parse().ok()?,
        filename: get_attr(e, "filename")?,
        physical_partition: get_attr(e, "physical_partition_number")?.parse().ok()?,
        size_in_bytes: get_attr(e, "size_in_bytes")?.parse().ok()?,
        start_sector: get_attr(e, "start_sector")?.parse().ok()?,
        value: get_attr(e, "value")?,
    })
}
