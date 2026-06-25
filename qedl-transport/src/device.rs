use crate::error::Result;
use std::time::Duration;

pub use qedl_core::DeviceInfo;

pub const QUALCOMM_VID: u16 = 0x05C6;
pub const QUALCOMM_9008_PID: u16 = 0x9008;
pub const QUALCOMM_90B8_PID: u16 = 0x90B8;

fn serialport_error_to_io(e: serialport::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
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
            if pid != QUALCOMM_9008_PID && pid != QUALCOMM_90B8_PID {
                continue;
            }

            let product_default = if pid == QUALCOMM_9008_PID {
                "Qualcomm 9008"
            } else {
                "Qualcomm 90B8"
            };
            let description = match &port.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    Some(info.product.as_deref().unwrap_or(product_default).to_string())
                }
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
            };
            tracing::debug!("Found Qualcomm device: {} (PID=0x{:04X})", info, pid);
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
            devices.push(DeviceInfo {
                port: port.port_name.clone(),
                serial,
                product: description.clone(),
                vid: vid.unwrap_or(0),
                pid: pid.unwrap_or(0),
                description,
            });
        }
        Ok(devices)
    }

    pub fn auto_select() -> Result<DeviceInfo> {
        let devices = Self::list()?;
        let has_9008 = devices.iter().any(|d| d.pid == QUALCOMM_9008_PID);
        let filtered: Vec<DeviceInfo> = if has_9008 {
            devices.into_iter().filter(|d| d.pid == QUALCOMM_9008_PID).collect()
        } else {
            devices
        };

        match filtered.len() {
            0 => {
                tracing::debug!("No 9008/90B8 device found via auto-select");
                Err(crate::error::TransportError::NotFound)
            }
            1 => {
                let device = filtered.into_iter().next().expect("filtered list is non-empty");
                tracing::debug!("Auto-selected device: {}", device);
                Ok(device)
            }
            n => {
                tracing::debug!("Multiple devices found ({}) via auto-select", n);
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
        tracing::info!("Waiting for 9008/90B8 device (timeout: {})...", timeout_desc);
        loop {
            let devices = Self::list_all()?;
            let matched = devices.into_iter().find(|d| {
                let port_ok = port.is_none_or(|p| d.port == p);
                let serial_ok = serial.is_none_or(|s| d.serial.as_deref() == Some(s));
                let qualcomm = d.vid == QUALCOMM_VID && (d.pid == QUALCOMM_9008_PID || d.pid == QUALCOMM_90B8_PID);
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

    /// Switch a 90B8 (DIAG mode) device to 9008 (EDL mode).
    /// Opens the DIAG serial port, sends the EDL mode switch command,
    /// then waits for the device to re-enumerate as 9008.
    pub fn switch_90b8_to_9008(port_name: &str, timeout_secs: u64) -> Result<()> {
        tracing::info!("Switching 90B8 DIAG device on {} to 9008 EDL mode...", port_name);

        let mut port = match serialport::new(port_name, 115200)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .flow_control(serialport::FlowControl::None)
            .timeout(std::time::Duration::from_millis(1000))
            .open()
        {
            Ok(p) => {
                tracing::debug!("Opened 90B8 port {}", port_name);
                p
            }
            Err(e) => {
                tracing::warn!("Failed to open 90B8 port {}: {}", port_name, e);
                return Err(crate::error::TransportError::Io(std::io::Error::other(e.to_string())));
            }
        };

        // Send EDL mode switch magic sequences
        // Common magics used by Qualcomm DIAG->EDL switch:
        let magics: &[&[u8]] = &[
            // Magic 1: Standard Sahara Hello (triggers EDL entry on some 90B8 devices)
            &[0x7E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E],
            // Magic 2: DIAG EDL command (\x75 = EDL mode cmd)
            &[0x75, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            // Magic 3: DLOAD mode switch
            &[0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        ];

        for (i, magic) in magics.iter().enumerate() {
            tracing::debug!("Sending EDL switch magic #{}: {:02X?}", i + 1, magic);
            let _ = port.write(magic);
            let _ = port.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        drop(port);
        tracing::info!("90B8 port closed, waiting for device to re-enumerate as 9008...");

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
