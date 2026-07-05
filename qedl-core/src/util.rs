use humansize::{DECIMAL, format_size};
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
    format_size(bytes, DECIMAL)
}

pub fn hex_dump(data: &[u8], max_bytes: usize) -> String {
    let len = data.len().min(max_bytes);
    let truncated = data.len() > max_bytes;
    let mut out = String::with_capacity(len * 4);

    for (i, chunk) in data[..len].chunks(16).enumerate() {
        let offset = i * 16;
        out.push_str(&format!("{:08x}  ", offset));

        for (j, &b) in chunk.iter().enumerate() {
            out.push_str(&format!("{:02x} ", b));
            if j == 7 {
                out.push(' ');
            }
        }

        if chunk.len() < 16 {
            for _ in 0..(16 - chunk.len()) {
                out.push_str("   ");
            }
            if chunk.len() < 8 {
                out.push(' ');
            }
        }

        out.push_str(" |");
        for &b in chunk {
            let c = if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            };
            out.push(c);
        }
        out.push_str("|\n");
    }

    if truncated {
        out.push_str(&format!(
            "... ({} bytes total, showing first {})\n",
            data.len(),
            max_bytes
        ));
    }

    out
}
