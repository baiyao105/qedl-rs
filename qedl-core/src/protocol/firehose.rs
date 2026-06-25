pub const TAG_CONFIGURE: &str = "configure";
pub const TAG_READ: &str = "read";
pub const TAG_PROGRAM: &str = "program";
pub const TAG_ERASE: &str = "erase";
pub const TAG_GETSTORAGEINFO: &str = "getstorageinfo";
pub const TAG_POWER: &str = "power";
pub const TAG_RESPONSE: &str = "response";
pub const TAG_DATA: &str = "data";
pub const TAG_LOG: &str = "log";

pub const VALUE_ACK: &str = "ACK";
pub const VALUE_NAK: &str = "NAK";

pub const ATTR_SECTOR_SIZE: &str = "SECTOR_SIZE_IN_BYTES";
pub const ATTR_NUM_SECTORS: &str = "num_partition_sectors";
pub const ATTR_PHYSICAL_PARTITION: &str = "physical_partition_number";
pub const ATTR_START_SECTOR: &str = "start_sector";
pub const ATTR_MEMORY_NAME: &str = "MemoryName";
pub const ATTR_MAX_PAYLOAD: &str = "MaxPayloadSizeToTargetInBytes";
pub const ATTR_VALUE: &str = "value";
pub const ATTR_RAWMODE: &str = "rawmode";

pub const DEFAULT_SECTOR_SIZE: u32 = 512;
pub const DEFAULT_MAX_PAYLOAD: u32 = 1024 * 1024;
