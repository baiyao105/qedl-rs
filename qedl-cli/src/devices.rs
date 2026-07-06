use owo_colors::OwoColorize;
use qedl::DeviceMode;
use qedl::transport::{DeviceEnumerator, DeviceInfo};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Serialize)]
struct JsonDevice {
    product: String,
    manufacturer: Option<String>,
    vid: u16,
    pid: u16,
    serial: Option<String>,
    location: Option<String>,
    mode: String,
    interfaces: Vec<JsonInterface>,
}

#[derive(Serialize)]
struct JsonInterface {
    mode: String,
    port: String,
    description: String,
}

#[derive(Serialize)]
struct JsonOutput {
    devices: Vec<JsonDevice>,
}

struct DeviceExtra {
    manufacturer: Option<String>,
    location: Option<String>,
}

struct DeviceGroup {
    serial: Option<String>,
    vid: u16,
    pid: u16,
    ports: Vec<(String, DeviceMode, String)>,
    extra: DeviceExtra,
}

fn query_usb_info(vid: u16, pid: u16) -> DeviceExtra {
    let Ok(devices) = rusb::DeviceList::new() else {
        return DeviceExtra {
            manufacturer: None,
            location: None,
        };
    };

    for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else { continue };
        if desc.vendor_id() != vid || desc.product_id() != pid {
            continue;
        }

        let location = Some(format!("USBROOT({})#USB({})", device.bus_number(), device.address()));

        let manufacturer = match device.open() {
            Ok(handle) => handle.read_manufacturer_string_ascii(&desc).ok().map(|s| s.to_string()),
            Err(_) => None,
        };

        return DeviceExtra { manufacturer, location };
    }

    DeviceExtra {
        manufacturer: None,
        location: None,
    }
}

fn group_devices(devices: Vec<DeviceInfo>) -> Vec<DeviceGroup> {
    let mut serial_map: HashMap<Option<String>, Vec<DeviceInfo>> = HashMap::new();
    for d in devices {
        serial_map.entry(d.serial.clone()).or_default().push(d);
    }

    let mut groups: Vec<DeviceGroup> = serial_map
        .into_values()
        .map(|mut ports| {
            ports.sort_by(|a, b| {
                fn mode_priority(m: &DeviceMode) -> u8 {
                    match m {
                        DeviceMode::Edl => 0,
                        DeviceMode::Diag => 1,
                        DeviceMode::Unknown => 2,
                    }
                }
                mode_priority(&a.mode).cmp(&mode_priority(&b.mode))
            });
            let first = &ports[0];
            let extra = query_usb_info(first.vid, first.pid);
            let serial = first.serial.clone();
            let vid = first.vid;
            let pid = first.pid;
            let port_list: Vec<(String, DeviceMode, String)> = ports
                .into_iter()
                .map(|d| {
                    let desc = d.description.unwrap_or_else(|| match d.mode {
                        DeviceMode::Edl => "Qualcomm 9008 (EDL)".to_string(),
                        DeviceMode::Diag => "Qualcomm DIAG".to_string(),
                        DeviceMode::Unknown => "Qualcomm Device".to_string(),
                    });
                    (d.port, d.mode, desc)
                })
                .collect();
            DeviceGroup {
                serial,
                vid,
                pid,
                ports: port_list,
                extra,
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        let a_edl = a.ports.iter().any(|(_, m, _)| *m == DeviceMode::Edl);
        let b_edl = b.ports.iter().any(|(_, m, _)| *m == DeviceMode::Edl);
        b_edl.cmp(&a_edl)
    });

    groups
}

fn mode_str(mode: DeviceMode) -> &'static str {
    match mode {
        DeviceMode::Edl => "EDL",
        DeviceMode::Diag => "DIAG",
        DeviceMode::Unknown => "UNKNOWN",
    }
}

fn title_str(group: &DeviceGroup) -> String {
    group.serial.clone().unwrap_or_else(|| "Unknown Device".to_string())
}

fn clean_desc(desc: &str) -> String {
    let s = desc.trim();
    if let Some(start) = s.rfind("(COM") {
        let after = &s[start + 4..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let rest = &after[digits.len()..];
            if rest.trim() == ")" {
                return s[..start].trim_end().to_string();
            }
        }
    }
    s.to_string()
}

fn format_tree(groups: &[DeviceGroup]) {
    if groups.is_empty() {
        println!("  {}", "No devices found.".dimmed());
        return;
    }

    println!("{} ({})", "Devices".cyan().bold(), groups.len());
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_group(group);
    }
}

fn print_group(group: &DeviceGroup) {
    let t = title_str(group);
    println!("{}", t.white().bold());

    if let Some(ref mfr) = group.extra.manufacturer {
        println!("{}  {:<14} {}", "├──".dimmed(), "Manufacturer".dimmed(), mfr);
    }
    println!(
        "{}  {:<14} {:04X}:{:04X}",
        "├──".dimmed(),
        "VID:PID".dimmed(),
        group.vid,
        group.pid
    );
    if let Some(ref loc) = group.extra.location {
        println!("{}  {:<14} {}", "├──".dimmed(), "Location".dimmed(), loc);
    }

    println!("{}  {}", "└──".dimmed(), "Interfaces".dimmed());
    for (i, (port, mode, desc)) in group.ports.iter().enumerate() {
        let is_last = i == group.ports.len() - 1;
        let branch = if is_last { "└──" } else { "├──" };
        let mode_label = match mode {
            DeviceMode::Edl => "EDL    ",
            DeviceMode::Diag => "DIAG   ",
            DeviceMode::Unknown => "UNKNOWN ",
        };
        println!(
            "     {} {} {}  {}  {:04X}:{:04X}",
            branch.dimmed(),
            port.white().bold(),
            mode_label.green(),
            clean_desc(desc),
            group.vid,
            group.pid
        );
    }
}

fn format_json(groups: &[DeviceGroup]) {
    let devices: Vec<JsonDevice> = groups
        .iter()
        .map(|group| JsonDevice {
            product: title_str(group),
            manufacturer: group.extra.manufacturer.clone(),
            vid: group.vid,
            pid: group.pid,
            serial: group.serial.clone(),
            location: group.extra.location.clone(),
            mode: "device".to_string(),
            interfaces: group
                .ports
                .iter()
                .map(|(port, mode, desc)| JsonInterface {
                    mode: mode_str(*mode).to_string(),
                    port: port.clone(),
                    description: desc.clone(),
                })
                .collect(),
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&JsonOutput { devices }).expect("JSON serialization failed")
    );
}

pub fn run_devices(watch: Option<u64>, json: bool) -> color_eyre::Result<()> {
    if let Some(interval_secs) = watch {
        run_watch(interval_secs)
    } else {
        let groups = group_devices(DeviceEnumerator::list()?);
        if json {
            format_json(&groups);
        } else {
            format_tree(&groups);
        }
        Ok(())
    }
}

fn run_watch(interval_secs: u64) -> color_eyre::Result<()> {
    use std::io::Write;

    eprintln!(
        "  {} (refreshing every {}s, Ctrl+C to stop)",
        "Device Monitor".cyan().bold(),
        interval_secs
    );

    let mut prev_keys: HashSet<String> = HashSet::new();
    let mut lines: usize = 0;

    loop {
        let groups = group_devices(DeviceEnumerator::list()?);

        let current_keys: HashSet<String> = groups
            .iter()
            .flat_map(|g| {
                g.ports
                    .iter()
                    .map(|(port, mode, _)| format!("{}:{}", port, mode_str(*mode)))
            })
            .collect();

        let removed: HashSet<String> = prev_keys.difference(&current_keys).cloned().collect();
        let added: HashSet<String> = current_keys.difference(&prev_keys).cloned().collect();
        prev_keys = current_keys;

        // Move cursor up and clear previous device lines
        if lines > 0 {
            eprint!("\x1B[{}F\x1B[J", lines);
        }

        let mut count = 0usize;

        if groups.is_empty() || groups.iter().all(|g| g.ports.is_empty()) {
            eprintln!("  {}", "No devices found.".dimmed());
            count += 1;
        } else {
            for group in &groups {
                for (port, mode, desc) in &group.ports {
                    let key = format!("{}:{}", port, mode_str(*mode));
                    let ch = if removed.contains(&key) {
                        '-'
                    } else if added.contains(&key) {
                        '+'
                    } else {
                        ' '
                    };
                    let prefix = match ch {
                        '+' => "[+]".to_string(),
                        '-' => "[-]".to_string(),
                        _ => "[ ]".to_string(),
                    };
                    let clean = clean_desc(desc);
                    eprintln!(
                        "{} {}  ({})  {}",
                        prefix,
                        port.white().bold(),
                        mode_str(*mode).cyan(),
                        clean.dimmed()
                    );
                    count += 1;
                }
            }
        }

        lines = count;
        std::io::stderr().flush()?;
        std::thread::sleep(Duration::from_secs(interval_secs));
    }
}
