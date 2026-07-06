use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use owo_colors::OwoColorize;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tracing_indicatif::span_ext::IndicatifSpanExt;

const DIVIDER: &str = "────────────────────────────────────────";

/// A simple spinner for showing temporary progress during device operations.
/// Uses tracing-indicatif's span-based progress so the IndicatifLayer coordinates
/// spinner redraws with log output on stderr, preventing residual text artifacts.
pub struct Spinner {
    notify: Arc<Notify>,
}

impl Spinner {
    /// Create a new spinner with a message, coordinated with tracing via IndicatifLayer
    pub fn new(message: &str) -> Self {
        let span = tracing::info_span!("spinner");
        span.pb_set_style(
            &ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        span.pb_set_message(message);

        let notify = Arc::new(Notify::new());
        let notify_clone = notify.clone();

        // Spawn a task that keeps the span entered so the progress bar stays visible.
        // The IndicatifLayer clears/redraws this progress bar around log messages.
        tokio::spawn(async move {
            let _guard = span.enter();
            notify_clone.notified().await;
            // When notified, _guard is dropped, exiting the span.
            // The IndicatifLayer finishes the progress bar automatically.
        });

        Self { notify }
    }
}

impl qedl::SpinnerHandle for Spinner {
    fn finish(&self) {
        self.notify.notify_one();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.notify.notify_one();
    }
}

/// Print a success message
pub fn success(msg: &str) {
    println!("{} {}", "[OK]".green().bold(), msg);
}

/// Print an error message
#[allow(dead_code)]
pub fn error(msg: &str) {
    println!("{} {}", "[FAIL]".red().bold(), msg);
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

/// Parse a hex string (0x prefix) or decimal string to u64
pub fn parse_hex_or_decimal(s: &str) -> color_eyre::Result<u64> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16).map_err(|e| color_eyre::eyre::eyre!("Invalid hex address '{}': {}", s, e))
    } else {
        s.parse::<u64>()
            .map_err(|e| color_eyre::eyre::eyre!("Invalid address '{}': {}", s, e))
    }
}

/// Parse hex data string. Supports formats:
/// - "0xAA 0xBB 0xCC" (space-separated with 0x prefix)
/// - "AABBCC" (continuous hex string)
/// - "AA BB CC" (space-separated without prefix)
pub fn parse_hex_data(s: &str) -> color_eyre::Result<Vec<u8>> {
    let s = s.trim();

    // Try continuous hex string first (no spaces, no 0x prefix)
    if !s.contains(' ') && !s.contains("0x") && !s.contains("0X") {
        if !s.len().is_multiple_of(2) {
            return Err(color_eyre::eyre::eyre!(
                "Hex string must have even number of characters: '{}'",
                s
            ));
        }
        let mut bytes = Vec::with_capacity(s.len() / 2);
        for i in (0..s.len()).step_by(2) {
            let byte = u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| color_eyre::eyre::eyre!("Invalid hex at position {}: {}", i, e))?;
            bytes.push(byte);
        }
        return Ok(bytes);
    }

    // Handle space-separated format (with or without 0x prefix)
    let mut bytes = Vec::new();
    for part in s.split_whitespace() {
        let hex_str = if part.starts_with("0x") || part.starts_with("0X") {
            &part[2..]
        } else {
            part
        };
        if hex_str.len() != 2 {
            return Err(color_eyre::eyre::eyre!(
                "Expected 2 hex characters per byte, got '{}'",
                part
            ));
        }
        let byte = u8::from_str_radix(hex_str, 16)
            .map_err(|e| color_eyre::eyre::eyre!("Invalid hex byte '{}': {}", part, e))?;
        bytes.push(byte);
    }

    if bytes.is_empty() {
        return Err(color_eyre::eyre::eyre!("No hex data provided"));
    }

    Ok(bytes)
}

/// Display hex dump of data
pub fn hex_dump_display(data: &[u8]) {
    const BYTES_PER_LINE: usize = 16;

    for (i, chunk) in data.chunks(BYTES_PER_LINE).enumerate() {
        let offset = i * BYTES_PER_LINE;

        // Hex part
        let hex: String = chunk
            .iter()
            .enumerate()
            .map(|(j, &b)| {
                if j == 8 {
                    format!("  {:02X}", b)
                } else {
                    format!(" {:02X}", b)
                }
            })
            .collect();

        // ASCII part
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        println!("    {:08X}  {:<49}  |{}|", offset, hex, ascii);
    }
}

pub struct IndicatifProgress {
    pb: Mutex<Option<ProgressBar>>,
}

impl IndicatifProgress {
    pub fn new() -> Self {
        Self { pb: Mutex::new(None) }
    }
}

impl Default for IndicatifProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl qedl::ProgressReporter for IndicatifProgress {
    fn start(&self, total: u64, _message: &str) {
        let style = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .expect("valid progress template")
            .progress_chars("#>-");
        let pb = ProgressBar::with_draw_target(Some(total), ProgressDrawTarget::stdout()).with_style(style);
        *self.pb.lock().unwrap() = Some(pb);
    }

    fn update(&self, current: u64) {
        if let Some(ref pb) = *self.pb.lock().unwrap() {
            pb.set_position(current);
        }
    }

    fn finish(&self, message: &str) {
        if let Some(pb) = self.pb.lock().unwrap().take() {
            pb.finish_with_message(message.to_string());
        }
    }
}
