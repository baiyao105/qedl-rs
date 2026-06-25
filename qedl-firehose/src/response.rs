use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug, Clone)]
pub struct FirehoseResponse {
    pub value: ResponseValue,
    pub raw_mode: bool,
    pub error_log: Option<String>,
    pub logs: Vec<String>,

    pub memory_name: Option<String>,
    pub sector_size: Option<u32>,
    pub max_payload_size: Option<u32>,
    pub total_sectors: Option<u64>,
    pub num_partition_sectors: Option<u64>,
    pub physical_partition_number: Option<u8>,
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
        let mut error_log: Option<String> = None;
        let mut logs: Vec<String> = Vec::new();
        let mut memory_name: Option<String> = None;
        let mut sector_size: Option<u32> = None;
        let mut max_payload_size: Option<u32> = None;
        let mut total_sectors: Option<u64> = None;
        let mut num_partition_sectors: Option<u64> = None;
        let mut physical_partition_number: Option<u8> = None;
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
                            error_log = Some(v.clone());
                            logs.push(v);
                        }
                    }

                    if tag_lower == "memory" || tag_lower == "memoryname" {
                        memory_name = get_attr(e, "name")
                            .or_else(|| get_attr(e, "MemoryName"))
                            .or_else(|| get_attr(e, "memory"));
                    }

                    if sector_size.is_none() {
                        sector_size = get_attr_u32(e, "SECTOR_SIZE_IN_BYTES")
                            .or_else(|| get_attr_u32(e, "sector_size"))
                            .or_else(|| get_attr_u32(e, "SectorSize"));
                    }
                    if max_payload_size.is_none() {
                        max_payload_size = get_attr_u32(e, "MaxPayloadSizeToTargetInBytes")
                            .or_else(|| get_attr_u32(e, "MaxPayloadSizeToTarget"))
                            .or_else(|| get_attr_u32(e, "max_payload_size"))
                            .or_else(|| get_attr_u32(e, "MaxPayloadSize"));
                    }
                    if total_sectors.is_none() {
                        total_sectors = get_attr_u64(e, "total_sectors")
                            .or_else(|| get_attr_u64(e, "TotalSectors"))
                            .or_else(|| get_attr_u64(e, "num_partition_sectors"));
                    }
                    if num_partition_sectors.is_none() {
                        num_partition_sectors = get_attr_u64(e, "num_partition_sectors");
                    }
                    if physical_partition_number.is_none() {
                        physical_partition_number = get_attr_u32(e, "physical_partition_number")
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
                            if error_log.is_none() {
                                error_log = Some(log_text.clone());
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
            error_log,
            logs,
            memory_name,
            sector_size,
            max_payload_size,
            total_sectors,
            num_partition_sectors,
            physical_partition_number,
        })
    }

    pub fn is_ack(&self) -> bool {
        self.value == ResponseValue::Ack
    }

    pub fn is_nak(&self) -> bool {
        self.value == ResponseValue::Nak
    }
}
