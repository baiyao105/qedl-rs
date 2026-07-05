use owo_colors::OwoColorize;

const DIVIDER: &str = "────────────────────────────────────────";

/// Print a success message
pub fn success(msg: &str) {
    println!("{} {}", "[OK]".green().bold(), msg);
}

/// Print an error message
pub fn error(msg: &str) {
    eprintln!("{} {}", "[FAIL]".red().bold(), msg);
}

/// Print an info message
pub fn info(msg: &str) {
    println!("  {}", msg);
}

/// Print a section header with divider
pub fn header(title: &str) {
    println!();
    println!("  {}", title.cyan().bold());
    println!("  {}", DIVIDER.dimmed());
}

/// Print a key-value pair with aligned columns
pub fn kv(key: &str, value: &str) {
    println!("    {:<14} {}", key.white().bold(), value);
}

/// Format and print device list
pub fn device_list(devices: &[String]) {
    if devices.is_empty() {
        println!("  {}", "No 9008/DIAG devices found.".dimmed());
        return;
    }
    println!("  {}", format!("Found {} device(s):", devices.len()).bold());
    println!();
    for (i, dev) in devices.iter().enumerate() {
        println!("    {} {}", format!("{}.", i + 1).dimmed(), dev);
    }
}

/// Print XML response
pub fn xml_response(success: bool, message: &str) {
    if success {
        println!("{} {}", "[OK]".green().bold(), message);
    } else {
        println!("{} {}", "[FAIL]".red().bold(), message);
    }
}
