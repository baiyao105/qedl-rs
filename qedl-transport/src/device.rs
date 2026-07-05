use crate::error::Result;
use qedl_core::{DeviceMode, hex_dump};
use std::collections::HashMap;
use std::time::Duration;

pub use qedl_core::DeviceInfo;

pub const QUALCOMM_VID: u16 = 0x05C6;
pub const QUALCOMM_9008_PID: u16 = 0x9008;

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

/// Query the USB interface descriptors for a device to determine its operating mode.
///
/// Qualcomm USB devices expose different interface class/subclass/protocol combos:
/// - EDL (firehose): class=0xFF, subclass=0xFF, protocol=0xFF
/// - DIAG:           class=0xFF, subclass=0xFF, protocol≠0xFF
///
/// Falls back to `DeviceMode::Unknown` if the device cannot be found or queried.
fn query_device_mode(vid: u16, pid: u16) -> DeviceMode {
    let devices = match rusb::DeviceList::new() {
        Ok(list) => list,
        Err(e) => {
            tracing::trace!("rusb: failed to enumerate devices: {}", e);
            return DeviceMode::Unknown;
        }
    };

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if desc.vendor_id() != vid || desc.product_id() != pid {
            continue;
        }

        let config = match device.active_config_descriptor() {
            Ok(c) => c,
            Err(e) => {
                tracing::trace!(
                    "rusb: failed to read config descriptor for {:04X}:{:04X}: {}",
                    vid,
                    pid,
                    e
                );
                return DeviceMode::Unknown;
            }
        };

        for iface in config.interfaces() {
            for iface_desc in iface.descriptors() {
                let class = iface_desc.class_code();
                let subclass = iface_desc.sub_class_code();
                let protocol = iface_desc.protocol_code();

                tracing::trace!(
                    "rusb: {:04X}:{:04X} interface {} class={:02X} subclass={:02X} protocol={:02X}",
                    vid,
                    pid,
                    iface_desc.interface_number(),
                    class,
                    subclass,
                    protocol
                );

                if class == 0xFF && subclass == 0xFF {
                    return if protocol == 0xFF {
                        DeviceMode::Edl
                    } else {
                        DeviceMode::Diag
                    };
                }
            }
        }

        tracing::trace!("rusb: {:04X}:{:04X} has no vendor-specific (0xFF) interface", vid, pid);
        return DeviceMode::Unknown;
    }

    DeviceMode::Unknown
}

pub struct DeviceEnumerator;

impl DeviceEnumerator {
    pub fn list() -> Result<Vec<DeviceInfo>> {
        tracing::debug!(
            "Scanning serial ports for Qualcomm devices (VID=0x{:04X})...",
            QUALCOMM_VID
        );
        let ports = serialport::available_ports().map_err(serialport_error_to_io)?;

        let mut devices = Vec::new();
        for port in &ports {
            let (vid, pid) = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => (info.vid, info.pid),
                _ => continue,
            };

            if vid != QUALCOMM_VID {
                continue;
            }

            let mode = query_device_mode(vid, pid);

            let description = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => Some(info.product.clone().unwrap_or_else(|| match mode {
                    DeviceMode::Edl => "Qualcomm 9008 (EDL)".to_string(),
                    DeviceMode::Diag => "Qualcomm DIAG".to_string(),
                    DeviceMode::Unknown => "Qualcomm".to_string(),
                })),
                _ => None,
            };
            let serial = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => info.serial_number.clone(),
                _ => None,
            };
            let info = DeviceInfo {
                port: port.port_name.clone(),
                serial,
                product: description.clone(),
                vid,
                pid,
                description,
                mode,
            };
            tracing::debug!("Found Qualcomm device: {} (PID=0x{:04X}, mode={:?})", info, pid, mode);
            devices.push(info);
        }
        tracing::debug!("Scan complete: {} Qualcomm device(s) found", devices.len());
        Ok(devices)
    }

    pub fn list_all() -> Result<Vec<DeviceInfo>> {
        let ports = serialport::available_ports().map_err(serialport_error_to_io)?;

        let mut devices = Vec::new();
        for port in &ports {
            let (vid, pid, serial, description) = match &port.port_type {
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
                query_device_mode(v, p)
            } else {
                DeviceMode::Unknown
            };
            devices.push(DeviceInfo {
                port: port.port_name.clone(),
                serial,
                product: description.clone(),
                vid: v,
                pid: p,
                description,
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

        // Prefer EDL over DIAG to avoid unnecessary mode switch
        let edl_groups: Vec<_> = devices
            .iter()
            .filter(|d| d.is_9008())
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .fold(Vec::<DeviceInfo>::new(), |mut acc, d| {
                if acc.iter().any(|e| e.serial == d.serial) {
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
                let qualcomm = d.vid == QUALCOMM_VID && (d.pid == QUALCOMM_9008_PID || d.is_diag());
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
            tracing::debug!(
                "Sending DIAG EDL switch command ({} baud):\n{}",
                baud,
                hex_dump(&frame, 64)
            );
            if port.write_all(&frame).is_err() || port.flush().is_err() {
                tracing::debug!("Failed to write at {} baud", baud);
                continue;
            }

            // Read and discard the response
            let mut buf = [0u8; 256];
            match port.read(&mut buf) {
                Ok(n) => {
                    tracing::debug!("DIAG response ({} bytes):\n{}", n, hex_dump(&buf[..n], 64));
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

        let skipped_port = port_name.to_string();

        let start = std::time::Instant::now();
        loop {
            let devices = Self::list()?;
            if devices.iter().any(|d| d.is_9008() && d.port != skipped_port) {
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
