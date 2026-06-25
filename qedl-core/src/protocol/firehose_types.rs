#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirehoseFunction {
    Program,
    Read,
    Erase,
    Peek,
    Poke,
    Nop,
    Unknown(String),
}

impl std::str::FromStr for FirehoseFunction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "program" => Self::Program,
            "read" => Self::Read,
            "erase" => Self::Erase,
            "peek" => Self::Peek,
            "poke" => Self::Poke,
            "nop" => Self::Nop,
            other => Self::Unknown(other.to_string()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FirehoseInfo {
    pub sector_size: u32,
    pub max_payload_size: u32,
    pub max_xml_size: Option<u32>,
    pub supported_functions: Vec<FirehoseFunction>,
}

impl Default for FirehoseInfo {
    fn default() -> Self {
        Self {
            sector_size: 512,
            max_payload_size: 1024 * 1024,
            max_xml_size: None,
            supported_functions: Vec::new(),
        }
    }
}

impl FirehoseInfo {
    pub fn supports(&self, func: &FirehoseFunction) -> bool {
        self.supported_functions.contains(func)
    }
}
