use crate::error::Result;
use qedl_core::DeviceMode;
#[cfg(feature = "trace-transport")]
use qedl_core::hex_dump;
use std::collections::HashMap;
use std::time::Duration;

pub use qedl_core::DeviceInfo;

pub const QUALCOMM_VID: u16 = 0x05C6;
pub const QUALCOMM_9008_PID: u16 = 0x9008;

/// Known Qualcomm EDL (emergency download) mode PIDs, used as fallback when
/// USB interface descriptors cannot be queried.
///
/// Common PIDs include 0x9008 (most SoCs), 0x900E (SM8450/SM8550/SM8650+), 0x900D.
pub const EDL_PIDS: &[u16] = &[0x9008, 0x900E, 0x900D];

/// Known Qualcomm DIAG mode PIDs (fallback when interface descriptor unavailable).
pub const DIAG_PIDS: &[u16] = &[0x90B8, 0x9091, 0x90E8];

fn serialport_error_to_io(e: serialport::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// CRC-16/CCITT (polynomial 0x11021, init 0xFFFF, xorOut 0xFFFF).
/// Used by Qualcomm DIAG HDLC framing.
fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFF
}

/// Escape payload for HDLC: 0x7E → 0x7D 0x5E, 0x7D → 0x7D 0x5D
fn hdlc_escape(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 16);
    for &b in payload {
        match b {
            0x7E => {
                out.push(0x7D);
                out.push(0x5E);
            }
            0x7D => {
                out.push(0x7D);
                out.push(0x5D);
            }
            _ => out.push(b),
        }
    }
    out
}

/// Build an HDLC-framed DIAG packet: 0x7E [escaped{cmd + payload + CRC16}] 0x7E
fn diag_frame(cmd: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + payload.len() + 2);
    data.push(cmd);
    data.extend_from_slice(payload);
    let crc = crc16_ccitt(&data);
    data.push((crc & 0xFF) as u8);
    data.push((crc >> 8) as u8);

    let escaped = hdlc_escape(&data);
    let mut frame = Vec::with_capacity(2 + escaped.len());
    frame.push(0x7E);
    frame.extend_from_slice(&escaped);
    frame.push(0x7E);
    frame
}

/// Determine the operating mode for each USB interface of a device.
///
/// Reads USB interface descriptors (class/subclass/protocol) plus iInterface
/// string descriptors to classify each interface:
///
/// | Condition | Mode |
/// |---|---|
/// | class=0xFF, subclass=0xFF, protocol=0xFF | `Edl` |
/// | class=0xFF, subclass=0xFF, protocol≠0xFF + iInterface contains "NMEA" or "GPS" | `Nmea` |
/// | class=0xFF, subclass=0xFF, protocol≠0xFF | `Diag` |
/// | class=0xFF, subclass=0x42 | `Adb` |
/// | class=0x02 or 0x0A | `Modem` |
/// | anything else | `Unknown` |
///
/// Also returns each interface's iInterface string descriptor for
/// matching against serialport's product string.
///
/// When `expected_serial` is provided, correlates the serial port
/// to the exact physical USB device by serial number.
fn query_per_interface_modes(
    vid: u16,
    pid: u16,
    expected_serial: Option<&str>,
) -> Vec<(u8, DeviceMode, Option<String>)> {
    let Ok(devices) = rusb::DeviceList::new() else {
        return Vec::new();
    };

    // Only known EDL PIDs can report Edl. protocol=0xFF on DIAG/Modem
    // devices is treated as Diag.
    const EDL_PIDS: &[u16] = &[0x9008, 0x900E, 0x900D];
    let is_edl_pid = EDL_PIDS.contains(&pid);

    let mut result: Vec<(u8, DeviceMode, Option<String>)> = Vec::new();

    'device_loop: for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else { continue };
        if desc.vendor_id() != vid || desc.product_id() != pid {
            continue;
        }

        // Serial number correlation
        if let Some(expected) = expected_serial {
            let timeout = Duration::from_millis(500);
            if let Ok(handle) = device.open() {
                let langs = handle.read_languages(timeout).unwrap_or_default();
                if let Some(&lang) = langs.first()
                    && let Ok(actual) = handle.read_serial_number_string(lang, &desc, timeout)
                    && actual != expected
                {
                    continue 'device_loop;
                }
            }
        }

        let Ok(config) = device.active_config_descriptor() else {
            continue;
        };

        for iface in config.interfaces() {
            for iface_desc in iface.descriptors() {
                let class = iface_desc.class_code();
                let subclass = iface_desc.sub_class_code();
                let protocol = iface_desc.protocol_code();
                let iface_num = iface_desc.interface_number();

                // Try to read iInterface string descriptor first (needed for NMEA detection)
                let iface_string = device.open().ok().and_then(|handle| {
                    let idx = iface_desc.description_string_index()?;
                    handle.read_string_descriptor_ascii(idx).ok()
                });

                let mode = match (class, subclass, protocol) {
                    // EDL only when PID is a known EDL PID
                    (0xFF, 0xFF, 0xFF) if is_edl_pid => DeviceMode::Edl,
                    (0xFF, 0x42, _) => DeviceMode::Adb,
                    (0x02, _, _) | (0x0A, _, _) => DeviceMode::Modem,
                    (0xFF, 0xFF, _) => {
                        // NMEA interfaces use vendor-specific class (0xFF) like DIAG,
                        // but their iInterface string contains "NMEA" or "GPS".
                        if let Some(ref s) = iface_string {
                            let lower = s.to_lowercase();
                            if lower.contains("nmea") || lower.contains("gps") {
                                DeviceMode::Nmea
                            } else {
                                DeviceMode::Diag
                            }
                        } else {
                            DeviceMode::Diag
                        }
                    }
                    _ => DeviceMode::Unknown,
                };

                tracing::trace!(
                    "rusb: {:04X}:{:04X} iface {} class={:02X} subclass={:02X} protocol={:02X} iface_str={:?} → {:?}",
                    vid,
                    pid,
                    iface_num,
                    class,
                    subclass,
                    protocol,
                    iface_string,
                    mode
                );

                result.push((iface_num, mode, iface_string));
            }
        }

        return result;
    }

    result
}

/// Trait abstracting device discovery.
/// Enables network devices, virtual devices, or custom selection logic.
pub trait DeviceEnumeratorTrait {
    /// List all Qualcomm devices (filtering by VID)
    fn list(&self) -> Result<Vec<DeviceInfo>>;

    /// List all devices without filtering
    fn list_all(&self) -> Result<Vec<DeviceInfo>>;

    /// Auto-select the best device
    fn auto_select(&self) -> Result<DeviceInfo>;

    /// Find device by port name
    fn find_by_port(&self, port: &str) -> Result<DeviceInfo>;

    /// Find device by serial number
    fn find_by_serial(&self, serial: &str) -> Result<DeviceInfo>;

    /// Wait for a device to appear
    fn wait_for_device(
        &self,
        port: Option<&str>,
        serial: Option<&str>,
        timeout_secs: Option<u64>,
        poll_interval_ms: u64,
    ) -> Result<DeviceInfo>;

    /// Switch DIAG device to EDL mode
    fn switch_diag_to_edl(&self, port_name: &str, timeout_secs: u64) -> Result<()>;
}

pub struct DeviceEnumerator;

impl DeviceEnumerator {
    #[allow(clippy::type_complexity)]
    pub fn list() -> Result<Vec<DeviceInfo>> {
        let ports = serialport::available_ports().map_err(serialport_error_to_io)?;

        let mut mode_cache: HashMap<(u16, u16, Option<String>), Vec<(u8, DeviceMode, Option<String>)>> = HashMap::new();
        let mut port_index: HashMap<(u16, u16, Option<String>), usize> = HashMap::new();

        let mut devices = Vec::new();
        for port in &ports {
            let (vid, pid) = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => (info.vid, info.pid),
                _ => continue,
            };

            if vid != QUALCOMM_VID {
                continue;
            }

            let serial = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => info.serial_number.clone(),
                _ => None,
            };
            let cache_key = (vid, pid, serial.clone());

            let iface_modes = mode_cache
                .entry(cache_key.clone())
                .or_insert_with(|| query_per_interface_modes(vid, pid, serial.as_deref()));
            let product = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => info.product.clone(),
                _ => None,
            };

            // Match port to USB interface by iInterface string. Try to find an
            // interface whose string matches the serialport product name. This
            // is more reliable than sequential assignment when COM port order
            // does not match USB interface number order.
            let idx = port_index.entry(cache_key).or_insert(0);
            let mode = if let Some(ref prod) = product {
                let cleaned = prod.trim().trim_end_matches(')');
                let matched = iface_modes.iter().find(|(_, _, s)| {
                    s.as_deref()
                        .is_some_and(|iface_str| iface_str == cleaned || prod.starts_with(iface_str))
                });
                if let Some(&(_, m, _)) = matched {
                    m
                } else if *idx < iface_modes.len() {
                    let m = iface_modes[*idx].1;
                    *idx += 1;
                    m
                } else {
                    DeviceMode::Unknown
                }
            } else if *idx < iface_modes.len() {
                let m = iface_modes[*idx].1;
                *idx += 1;
                m
            } else {
                DeviceMode::Unknown
            };
            let description = product.clone().unwrap_or_else(|| match mode {
                DeviceMode::Edl => "Qualcomm 9008 (EDL)".to_string(),
                DeviceMode::Diag => "Qualcomm DIAG".to_string(),
                DeviceMode::Modem => "Qualcomm Modem".to_string(),
                DeviceMode::Nmea => "Qualcomm NMEA".to_string(),
                DeviceMode::Adb => "Qualcomm ADB".to_string(),
                DeviceMode::Unknown => "Qualcomm".to_string(),
            });
            let info = DeviceInfo {
                port: port.port_name.clone(),
                serial,
                product: Some(description.clone()),
                vid,
                pid,
                description: Some(description),
                mode,
            };
            devices.push(info);
        }
        Ok(devices)
    }

    #[allow(clippy::type_complexity)]
    pub fn list_all() -> Result<Vec<DeviceInfo>> {
        let ports = serialport::available_ports().map_err(serialport_error_to_io)?;

        let mut mode_cache: HashMap<(u16, u16, Option<String>), Vec<(u8, DeviceMode, Option<String>)>> = HashMap::new();
        let mut port_index: HashMap<(u16, u16, Option<String>), usize> = HashMap::new();

        let mut devices = Vec::new();
        for port in &ports {
            let (vid, pid, serial, product) = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => (
                    Some(info.vid),
                    Some(info.pid),
                    info.serial_number.clone(),
                    info.product.clone(),
                ),
                _ => (None, None, None, None),
            };
            let v = vid.unwrap_or(0);
            let p = pid.unwrap_or(0);

            let mode = if v == QUALCOMM_VID {
                let cache_key = (v, p, serial.clone());
                let iface_modes = mode_cache
                    .entry(cache_key.clone())
                    .or_insert_with(|| query_per_interface_modes(v, p, serial.as_deref()));
                let idx = port_index.entry(cache_key).or_insert(0);
                if iface_modes.is_empty() {
                    DeviceMode::Unknown
                } else if *idx < iface_modes.len() {
                    let m = iface_modes[*idx].1;
                    *idx += 1;
                    m
                } else {
                    *idx += 1;
                    DeviceMode::Unknown
                }
            } else {
                DeviceMode::Unknown
            };

            devices.push(DeviceInfo {
                port: port.port_name.clone(),
                serial,
                product: product.clone(),
                vid: v,
                pid: p,
                description: product.clone(),
                mode,
            });
        }
        Ok(devices)
    }

    /// Group devices by serial number. Devices with the same serial are
    /// different COM ports on the same physical Qualcomm composite device.
    fn group_by_serial(devices: Vec<DeviceInfo>) -> Vec<Vec<DeviceInfo>> {
        let mut map: HashMap<Option<String>, Vec<DeviceInfo>> = HashMap::new();
        for d in devices {
            map.entry(d.serial.clone()).or_default().push(d);
        }
        map.into_values().collect()
    }

    /// Pick the best DIAG port from a multi-port group.
    /// MDM may not support mode switching.
    fn select_diag_port(group: &[DeviceInfo]) -> DeviceInfo {
        let preferred = ["MSM Diagnostics", "Diagnostics", "MDM Diagnostics"];
        for name in &preferred {
            if let Some(d) = group
                .iter()
                .find(|d| d.description.as_deref().is_some_and(|desc| desc.contains(name)))
            {
                return d.clone();
            }
        }
        group.first().unwrap().clone()
    }

    pub fn auto_select() -> Result<DeviceInfo> {
        let devices = Self::list()?;

        // Prefer EDL over DIAG to avoid unnecessary mode switch.
        // Dedup by serial number only when serial is available — two serial-less
        // devices are treated as potentially distinct, preventing silent
        // collapse when multiple Qualcomm devices are connected without serials.
        let edl_groups: Vec<_> = devices
            .iter()
            .filter(|d| d.is_9008())
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .fold(Vec::<DeviceInfo>::new(), |mut acc, d| {
                if d.serial.is_some() && acc.iter().any(|e| e.serial == d.serial) {
                    return acc;
                }
                acc.push(d);
                acc
            });

        let edl_count = edl_groups.len();
        if edl_count == 1 {
            let device = edl_groups.into_iter().next().unwrap();
            tracing::debug!("Auto-selected EDL device: {}", device);
            return Ok(device);
        } else if edl_count > 1 {
            tracing::debug!("Multiple EDL devices found ({})", edl_count);
            return Err(crate::error::TransportError::MultipleFound { count: edl_count });
        }

        // Group DIAG devices by serial (multi-port composite devices)
        let diag: Vec<_> = devices.into_iter().filter(|d| d.is_diag()).collect();
        let groups = Self::group_by_serial(diag);

        match groups.len() {
            0 => {
                tracing::debug!("No Qualcomm device found via auto-select");
                Err(crate::error::TransportError::NotFound)
            }
            1 => {
                let group = groups.into_iter().next().unwrap();
                let device = Self::select_diag_port(&group);
                tracing::debug!("Auto-selected DIAG device: {} (from {} port(s))", device, group.len());
                Ok(device)
            }
            n => {
                tracing::debug!("Multiple independent DIAG devices found ({})", n);
                Err(crate::error::TransportError::MultipleFound { count: n })
            }
        }
    }

    pub fn find_by_port(port: &str) -> Result<DeviceInfo> {
        let devices = Self::list_all()?;
        devices
            .into_iter()
            .find(|d| d.port == port)
            .ok_or_else(|| crate::error::TransportError::InvalidPort(port.to_string()))
    }

    pub fn find_by_serial(serial: &str) -> Result<DeviceInfo> {
        let devices = Self::list()?;
        devices
            .into_iter()
            .find(|d| d.serial.as_deref() == Some(serial))
            .ok_or(crate::error::TransportError::NotFound)
    }

    pub fn wait_for_device(
        port: Option<&str>,
        serial: Option<&str>,
        timeout_secs: Option<u64>,
        poll_interval_ms: u64,
    ) -> Result<DeviceInfo> {
        let start = std::time::Instant::now();
        let timeout_desc = match timeout_secs {
            Some(s) => format!("{}s", s),
            None => "forever".to_string(),
        };
        tracing::info!("Waiting for 9008/DIAG device (timeout: {})...", timeout_desc);
        loop {
            let devices = Self::list_all()?;
            let matched = devices.into_iter().find(|d| {
                let port_ok = port.is_none_or(|p| d.port == p);
                let serial_ok = serial.is_none_or(|s| d.serial.as_deref() == Some(s));
                let qualcomm = d.vid == QUALCOMM_VID && (d.is_9008() || d.is_diag());
                port_ok && serial_ok && qualcomm
            });

            if let Some(device) = matched {
                tracing::info!(
                    "Device found after {:.1}s: {} (PID=0x{:04X})",
                    start.elapsed().as_secs_f64(),
                    device,
                    device.pid
                );
                return Ok(device);
            }

            if let Some(timeout) = timeout_secs
                && start.elapsed().as_secs() >= timeout
            {
                tracing::warn!("Timed out waiting for Qualcomm device after {}s", timeout);
                return Err(crate::error::TransportError::NotFound);
            }

            std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
        }
    }

    /// Switch a DIAG mode device to 9008 (EDL mode).
    /// Sends the DIAG subsystem command to enter Sahara/EDL mode,
    /// then waits for the device to re-enumerate as 9008.
    /// Tries both 115200 and 921600 baud rates.
    pub fn switch_diag_to_edl(port_name: &str, timeout_secs: u64) -> Result<()> {
        tracing::info!("Switching DIAG device on {} to 9008 EDL mode...", port_name);

        let baud_rates = [115200u32, 921600];

        let mut sent = false;
        for &baud in &baud_rates {
            tracing::debug!("Trying baud rate {} on {}", baud, port_name);
            let mut port = match serialport::new(port_name, baud)
                .data_bits(serialport::DataBits::Eight)
                .stop_bits(serialport::StopBits::One)
                .parity(serialport::Parity::None)
                .flow_control(serialport::FlowControl::None)
                .timeout(std::time::Duration::from_millis(2000))
                .open()
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!("Failed to open {} at {}: {}", port_name, baud, e);
                    continue;
                }
            };

            // DIAG_SUBSYS_CMD_F (0x4B) + Sahara subsystem (0x65) + switch cmd (0x01 LE)
            let frame = diag_frame(0x4B, &[0x65, 0x01, 0x00]);
            #[cfg(feature = "trace-transport")]
            tracing::debug!(
                "Sending DIAG EDL switch command ({} baud):\n{}",
                baud,
                hex_dump(&frame, 64)
            );
            #[cfg(not(feature = "trace-transport"))]
            tracing::debug!("Sending DIAG EDL switch command ({} baud, {} bytes)", baud, frame.len());
            if port.write_all(&frame).is_err() || port.flush().is_err() {
                tracing::debug!("Failed to write at {} baud", baud);
                continue;
            }

            // Read and discard the response
            let mut buf = [0u8; 256];
            match port.read(&mut buf) {
                Ok(n) => {
                    #[cfg(feature = "trace-transport")]
                    tracing::debug!("DIAG response ({} bytes):\n{}", n, hex_dump(&buf[..n], 64));
                    #[cfg(not(feature = "trace-transport"))]
                    tracing::debug!("DIAG response ({} bytes)", n);
                }
                Err(e) => {
                    tracing::debug!("No DIAG response at {}: {}", baud, e);
                }
            }

            sent = true;
            drop(port);
            break;
        }

        if !sent {
            tracing::warn!("Failed to open DIAG port at any baud rate");
            return Err(crate::error::TransportError::Io(std::io::Error::other(
                "failed to open DIAG port at any baud rate",
            )));
        }

        tracing::info!("DIAG port closed, waiting for device to re-enumerate as 9008...");

        // Record which 9008 devices were already present before the switch.
        // After the switch, we wait for a *new* 9008 device to appear, which
        // allows us to detect the switched device even when the OS re-uses
        // the same COM port number (common on Linux with stable udev rules).
        let preexisting_ports: Vec<String> = Self::list()
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.is_9008())
            .map(|d| d.port.clone())
            .collect();

        let start = std::time::Instant::now();
        loop {
            let devices = Self::list()?;
            if devices
                .iter()
                .any(|d| d.is_9008() && !preexisting_ports.contains(&d.port))
            {
                tracing::info!(
                    "Device re-enumerated as 9008 after {:.1}s",
                    start.elapsed().as_secs_f64()
                );
                return Ok(());
            }

            if start.elapsed().as_secs() >= timeout_secs {
                tracing::warn!("Timed out waiting for 9008 re-enumeration after {}s", timeout_secs);
                return Err(crate::error::TransportError::NotFound);
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    }
}

impl DeviceEnumeratorTrait for DeviceEnumerator {
    fn list(&self) -> Result<Vec<DeviceInfo>> {
        Self::list()
    }
    fn list_all(&self) -> Result<Vec<DeviceInfo>> {
        Self::list_all()
    }
    fn auto_select(&self) -> Result<DeviceInfo> {
        Self::auto_select()
    }
    fn find_by_port(&self, port: &str) -> Result<DeviceInfo> {
        Self::find_by_port(port)
    }
    fn find_by_serial(&self, serial: &str) -> Result<DeviceInfo> {
        Self::find_by_serial(serial)
    }
    fn wait_for_device(
        &self,
        port: Option<&str>,
        serial: Option<&str>,
        timeout_secs: Option<u64>,
        poll_interval_ms: u64,
    ) -> Result<DeviceInfo> {
        Self::wait_for_device(port, serial, timeout_secs, poll_interval_ms)
    }
    fn switch_diag_to_edl(&self, port_name: &str, timeout_secs: u64) -> Result<()> {
        Self::switch_diag_to_edl(port_name, timeout_secs)
    }
}
