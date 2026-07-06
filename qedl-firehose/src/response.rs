use quick_xml::Reader;
use quick_xml::events::Event;

/// Device configuration returned by Firehose configure and getstorageinfo responses.
#[derive(Debug, Clone, Default)]
pub struct FirehoseConfig {
    pub memory_name: Option<String>,
    pub sector_size: Option<u32>,
    pub max_payload_size: Option<u32>,
    pub max_payload_size_from_target: Option<u32>,
    pub max_payload_size_to_target_supported: Option<u32>,
    pub max_xml_size: Option<u32>,
    pub target_name: Option<String>,
    pub version: Option<String>,
    pub total_sectors: Option<u64>,
    pub num_partition_sectors: Option<u64>,
    pub physical_partition_number: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct FirehoseResponse {
    pub value: ResponseValue,
    pub raw_mode: bool,
    /// Error message from NAK response. Only populated when value is Nak.
    pub error: Option<String>,
    /// Log messages from `<log>` tags. Printed at TRACE level.
    pub logs: Vec<String>,
    /// Device configuration fields (only populated by configure/getstorageinfo responses).
    pub config: FirehoseConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseValue {
    Ack,
    Nak,
}

use qedl_core::util::{get_attr, get_attr_u32, get_attr_u64};

impl FirehoseResponse {
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let mut reader = Reader::from_str(xml);

        let mut value = ResponseValue::Ack;
        let mut raw_mode = false;
        let mut error: Option<String> = None;
        let mut logs: Vec<String> = Vec::new();
        let mut config = FirehoseConfig::default();
        let mut in_log = false;
        let mut log_text = String::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = e.name();
                    let tag_bytes = tag_name.as_ref();
                    let tag_lower = String::from_utf8_lossy(tag_bytes).to_lowercase();

                    if tag_lower == "response" || tag_lower == "data" {
                        if let Some(v) = get_attr(e, "value") {
                            match v.as_str() {
                                "ACK" => value = ResponseValue::Ack,
                                "NAK" => value = ResponseValue::Nak,
                                _ => {}
                            }
                        }
                        if let Some(v) = get_attr(e, "rawmode") {
                            raw_mode = v == "true";
                        }
                    }

                    if tag_lower == "log" {
                        in_log = true;
                        log_text.clear();
                        if let Some(v) = get_attr(e, "value") {
                            // <log value="..."> attribute is the error message for NAK
                            if error.is_none() {
                                error = Some(v.clone());
                            }
                            logs.push(v);
                        }
                    }

                    if tag_lower == "memory" || tag_lower == "memoryname" {
                        config.memory_name = get_attr(e, "name")
                            .or_else(|| get_attr(e, "MemoryName"))
                            .or_else(|| get_attr(e, "memory"));
                    }

                    if config.sector_size.is_none() {
                        config.sector_size = get_attr_u32(e, "SECTOR_SIZE_IN_BYTES")
                            .or_else(|| get_attr_u32(e, "sector_size"))
                            .or_else(|| get_attr_u32(e, "SectorSize"));
                    }
                    if config.max_payload_size.is_none() {
                        config.max_payload_size = get_attr_u32(e, "MaxPayloadSizeToTargetInBytes")
                            .or_else(|| get_attr_u32(e, "MaxPayloadSizeToTarget"))
                            .or_else(|| get_attr_u32(e, "max_payload_size"))
                            .or_else(|| get_attr_u32(e, "MaxPayloadSize"));
                    }
                    if config.max_payload_size_from_target.is_none() {
                        config.max_payload_size_from_target = get_attr_u32(e, "MaxPayloadSizeFromTargetInBytes");
                    }
                    if config.max_payload_size_to_target_supported.is_none() {
                        config.max_payload_size_to_target_supported =
                            get_attr_u32(e, "MaxPayloadSizeToTargetInBytesSupported");
                    }
                    if config.max_xml_size.is_none() {
                        config.max_xml_size = get_attr_u32(e, "MaxXMLSizeInBytes");
                    }
                    if config.target_name.is_none() {
                        config.target_name = get_attr(e, "TargetName");
                    }
                    if config.version.is_none() {
                        config.version = get_attr(e, "Version");
                    }
                    if config.total_sectors.is_none() {
                        config.total_sectors = get_attr_u64(e, "total_sectors")
                            .or_else(|| get_attr_u64(e, "TotalSectors"))
                            .or_else(|| get_attr_u64(e, "num_partition_sectors"));
                    }
                    if config.num_partition_sectors.is_none() {
                        config.num_partition_sectors = get_attr_u64(e, "num_partition_sectors");
                    }
                    if config.physical_partition_number.is_none() {
                        config.physical_partition_number = get_attr_u32(e, "physical_partition_number")
                            .or_else(|| get_attr_u32(e, "PhysicalPartitionNumber"))
                            .map(|v| v as u8);
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_log && let Ok(t) = e.unescape() {
                        log_text.push_str(&t);
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag_name = e.name();
                    let tag_lower = String::from_utf8_lossy(tag_name.as_ref()).to_lowercase();
                    if tag_lower == "log" {
                        if in_log && !log_text.is_empty() {
                            logs.push(log_text.clone());
                            // Text content of <log> tag is also an error for NAK
                            if error.is_none() {
                                error = Some(log_text.clone());
                            }
                        }
                        in_log = false;
                        log_text.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(format!("XML parse error: {}", e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(Self {
            value,
            raw_mode,
            error,
            logs,
            config,
        })
    }

    pub fn is_ack(&self) -> bool {
        self.value == ResponseValue::Ack
    }

    pub fn is_nak(&self) -> bool {
        self.value == ResponseValue::Nak
    }

    pub fn memory_name(&self) -> Option<&str> {
        self.config.memory_name.as_deref()
    }

    pub fn sector_size(&self) -> Option<u32> {
        self.config.sector_size
    }

    pub fn total_sectors(&self) -> Option<u64> {
        self.config.total_sectors
    }

    pub fn max_payload_size(&self) -> Option<u32> {
        self.config.max_payload_size
    }

    pub fn max_payload_size_from_target(&self) -> Option<u32> {
        self.config.max_payload_size_from_target
    }

    pub fn max_payload_size_to_target_supported(&self) -> Option<u32> {
        self.config.max_payload_size_to_target_supported
    }

    pub fn max_xml_size(&self) -> Option<u32> {
        self.config.max_xml_size
    }

    pub fn target_name(&self) -> Option<&str> {
        self.config.target_name.as_deref()
    }

    pub fn version(&self) -> Option<&str> {
        self.config.version.as_deref()
    }

    pub fn num_partition_sectors(&self) -> Option<u64> {
        self.config.num_partition_sectors
    }

    pub fn physical_partition_number(&self) -> Option<u8> {
        self.config.physical_partition_number
    }
}
