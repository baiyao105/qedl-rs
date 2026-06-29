use crate::error::Result;
use qedl_core::DeviceMode;
use std::time::Duration;

pub use qedl_core::DeviceInfo;

pub const QUALCOMM_VID: u16 = 0x05C6;
pub const QUALCOMM_9008_PID: u16 = 0x9008;

fn serialport_error_to_io(e: serialport::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
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

    pub fn auto_select() -> Result<DeviceInfo> {
        let devices = Self::list()?;

        // Prefer EDL over DIAG to avoid unnecessary mode switch
        let edl: Vec<_> = devices.iter().filter(|d| d.is_9008()).collect();
        if !edl.is_empty() {
            if edl.len() == 1 {
                let device = edl.into_iter().next().unwrap().clone();
                tracing::debug!("Auto-selected EDL device: {}", device);
                return Ok(device);
            } else {
                tracing::debug!("Multiple EDL devices found ({})", edl.len());
                return Err(crate::error::TransportError::MultipleFound { count: edl.len() });
            }
        }

        let diag: Vec<_> = devices.into_iter().filter(|d| d.is_diag()).collect();
        match diag.len() {
            0 => {
                tracing::debug!("No Qualcomm device found via auto-select");
                Err(crate::error::TransportError::NotFound)
            }
            1 => {
                let device = diag.into_iter().next().unwrap();
                tracing::debug!("Auto-selected DIAG device: {}", device);
                Ok(device)
            }
            n => {
                tracing::debug!("Multiple DIAG devices found ({})", n);
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
    /// Opens the DIAG serial port, sends the EDL mode switch command,
    /// then waits for the device to re-enumerate as 9008.
    pub fn switch_diag_to_edl(port_name: &str, timeout_secs: u64) -> Result<()> {
        tracing::info!("Switching DIAG device on {} to 9008 EDL mode...", port_name);

        let mut port = match serialport::new(port_name, 115200)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .flow_control(serialport::FlowControl::None)
            .timeout(std::time::Duration::from_millis(1000))
            .open()
        {
            Ok(p) => {
                tracing::debug!("Opened DIAG port {}", port_name);
                p
            }
            Err(e) => {
                tracing::warn!("Failed to open DIAG port {}: {}", port_name, e);
                return Err(crate::error::TransportError::Io(std::io::Error::other(e.to_string())));
            }
        };

        // API: Qualcomm DIAG->EDL switch magic sequences (device-specific, order matters)
        let magics: &[&[u8]] = &[
            &[0x7E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E], // Sahara Hello
            &[0x75, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],       // DIAG EDL cmd
            &[0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],       // DLOAD switch
        ];

        for (i, magic) in magics.iter().enumerate() {
            tracing::debug!("Sending EDL switch magic #{}: {:02X?}", i + 1, magic);
            let _ = port.write(magic);
            let _ = port.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        drop(port);
        tracing::info!("DIAG port closed, waiting for device to re-enumerate as 9008...");

        let start = std::time::Instant::now();
        loop {
            let devices = Self::list()?;
            if devices.iter().any(|d| d.is_9008()) {
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
