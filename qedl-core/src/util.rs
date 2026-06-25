use quick_xml::events::BytesStart;

pub fn get_attr(e: &BytesStart, name: &str) -> Option<String> {
    e.try_get_attribute(name)
        .ok()
        .flatten()
        .and_then(|a| a.unescape_value().ok())
        .map(|c| c.to_string())
}

pub fn get_attr_u32(e: &BytesStart, name: &str) -> Option<u32> {
    get_attr(e, name).and_then(|v| v.parse().ok())
}

pub fn get_attr_u64(e: &BytesStart, name: &str) -> Option<u64> {
    get_attr(e, name).and_then(|v| v.parse().ok())
}

pub fn humanize_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
