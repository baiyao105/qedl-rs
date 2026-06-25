use quick_xml::Reader;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use std::path::Path;

#[derive(Debug)]
pub enum ParseError {
    Xml(quick_xml::Error),
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    MissingAttribute(String),
    InvalidValue(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "XML error: {}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Utf8(e) => write!(f, "UTF-8 error: {}", e),
            Self::MissingAttribute(s) => write!(f, "Missing attribute: {}", s),
            Self::InvalidValue(s) => write!(f, "Invalid value: {}", s),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<quick_xml::Error> for ParseError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Xml(e)
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<std::string::FromUtf8Error> for ParseError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Self::Utf8(e)
    }
}

#[derive(Debug, Clone)]
pub struct RawEntry {
    pub sector_start: u64,
    pub num_sectors: u64,
    pub file: String,
    pub physical_partition: u8,
    pub sparse: bool,
    pub read_back_verify: bool,
    pub burst_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RawProgram {
    pub entries: Vec<RawEntry>,
}

impl RawProgram {
    pub fn parse_file(path: &Path) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_str(&content)
    }

    pub fn parse_str(xml: &str) -> Result<Self, ParseError> {
        let mut reader = Reader::from_str(xml);
        let mut entries = Vec::new();
        let mut current_entry: Option<RawEntry> = None;

        loop {
            match reader.read_event()? {
                XmlEvent::Start(ref e) if e.name() == QName(b"program") => {
                    current_entry = Some(RawEntry {
                        sector_start: 0,
                        num_sectors: 0,
                        file: String::new(),
                        physical_partition: 0,
                        sparse: false,
                        read_back_verify: false,
                        burst_size: None,
                    });
                    for attr in e.attributes().with_checks(false) {
                        match attr {
                            Ok(attr) => {
                                if let Some(ref mut entry) = current_entry {
                                    Self::parse_attribute(entry, &attr)?;
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                }
                XmlEvent::Empty(ref e) if e.name() == QName(b"program") => {
                    let entry = Self::parse_program_tag(e)?;
                    entries.push(entry);
                }
                XmlEvent::End(ref e) if e.name() == QName(b"program") => {
                    if let Some(entry) = current_entry.take() {
                        entries.push(entry);
                    }
                }
                XmlEvent::Eof => break,
                _ => {}
            }
        }

        Ok(Self { entries })
    }

    fn attr_str(attr: &quick_xml::events::attributes::Attribute) -> Result<String, ParseError> {
        Ok(String::from_utf8(attr.value.to_vec())?)
    }

    fn parse_attribute(
        entry: &mut RawEntry,
        attr: &quick_xml::events::attributes::Attribute,
    ) -> Result<(), ParseError> {
        match attr.key {
            QName(b"SECTOR_START") => {
                let v = Self::attr_str(attr)?;
                entry.sector_start = v.parse().map_err(|e| ParseError::InvalidValue(format!("{}", e)))?;
            }
            QName(b"NUM_SECTORS") => {
                let v = Self::attr_str(attr)?;
                entry.num_sectors = v.parse().map_err(|e| ParseError::InvalidValue(format!("{}", e)))?;
            }
            QName(b"file") => {
                entry.file = Self::attr_str(attr)?;
            }
            QName(b"physical_partition_number") => {
                let v = Self::attr_str(attr)?;
                entry.physical_partition = v.parse().map_err(|e| ParseError::InvalidValue(format!("{}", e)))?;
            }
            QName(b"sparse") => {
                entry.sparse = Self::attr_str(attr)? == "true";
            }
            QName(b"read_back_verify") => {
                entry.read_back_verify = Self::attr_str(attr)? == "true";
            }
            QName(b"burst_size") => {
                let v = Self::attr_str(attr)?;
                entry.burst_size = Some(v.parse().map_err(|e| ParseError::InvalidValue(format!("{}", e)))?);
            }
            _ => {}
        }
        Ok(())
    }

    fn parse_program_tag(e: &quick_xml::events::BytesStart) -> Result<RawEntry, ParseError> {
        let mut entry = RawEntry {
            sector_start: 0,
            num_sectors: 0,
            file: String::new(),
            physical_partition: 0,
            sparse: false,
            read_back_verify: false,
            burst_size: None,
        };

        for attr in e.attributes().with_checks(false) {
            match attr {
                Ok(attr) => Self::parse_attribute(&mut entry, &attr)?,
                Err(_) => continue,
            }
        }

        Ok(entry)
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.file.is_empty() {
                errors.push(format!("Entry {}: missing file", i));
            }
            if entry.num_sectors == 0 {
                errors.push(format!("Entry {}: NUM_SECTORS is 0", i));
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
