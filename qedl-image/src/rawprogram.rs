use crate::error::{ImageError, Result};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    Program,
    Erase,
}

#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub task_type: TaskType,
    pub sector_size: u32,
    pub start_sector: u64,
    pub num_sectors: u64,
    pub physical_partition: u8,
    pub filename: Option<String>,
    pub label: Option<String>,
    pub sparse: bool,
    pub file_sector_offset: u64,
    pub size_in_kb: u64,
}

impl TaskEntry {
    pub fn image_path(&self, image_dir: &Path) -> Option<PathBuf> {
        self.filename.as_ref().map(|f| image_dir.join(f))
    }
}

#[derive(Debug, Clone)]
pub struct TaskList {
    pub entries: Vec<TaskEntry>,
    pub source_file: PathBuf,
}

use qedl_core::util::get_attr;

impl TaskList {
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
        let mut list = TaskList::new(source.to_path_buf());
        let mut reader = Reader::from_str(xml);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let tag_lower = tag.to_lowercase();

                    if tag_lower == "program" {
                        if let Some(entry) = parse_program_entry(e) {
                            list.entries.push(entry);
                        }
                    } else if tag_lower == "erase"
                        && let Some(entry) = parse_erase_entry(e)
                    {
                        list.entries.push(entry);
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

        Ok(list)
    }

    pub fn validate_files(&self, image_dir: &Path) -> Result<()> {
        for entry in &self.entries {
            if let Some(path) = entry.image_path(image_dir)
                && !path.exists()
            {
                return Err(ImageError::FileNotFound { expected: path });
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn parse_program_entry(e: &quick_xml::events::BytesStart) -> Option<TaskEntry> {
    Some(TaskEntry {
        task_type: TaskType::Program,
        sector_size: get_attr(e, "SECTOR_SIZE_IN_BYTES")?.parse().ok()?,
        start_sector: get_attr(e, "start_sector")?.parse().ok()?,
        num_sectors: get_attr(e, "num_partition_sectors")?.parse().ok()?,
        physical_partition: get_attr(e, "physical_partition_number")?.parse().ok()?,
        filename: get_attr(e, "filename"),
        label: get_attr(e, "label"),
        sparse: get_attr(e, "sparse").map(|s| s == "true").unwrap_or(false),
        file_sector_offset: get_attr(e, "file_sector_offset")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        size_in_kb: get_attr(e, "size_in_KB").and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}

fn parse_erase_entry(e: &quick_xml::events::BytesStart) -> Option<TaskEntry> {
    Some(TaskEntry {
        task_type: TaskType::Erase,
        sector_size: get_attr(e, "SECTOR_SIZE_IN_BYTES")?.parse().ok()?,
        start_sector: get_attr(e, "start_sector")?.parse().ok()?,
        num_sectors: get_attr(e, "num_partition_sectors")?.parse().ok()?,
        physical_partition: get_attr(e, "physical_partition_number")?.parse().ok()?,
        filename: None,
        label: get_attr(e, "label"),
        sparse: false,
        file_sector_offset: 0,
        size_in_kb: 0,
    })
}
