use crate::port::Transport;
use async_trait::async_trait;
use std::io::{self, Read, Write};
use std::sync::Mutex;
use std::time::Duration;

pub struct SerialTransport {
    port: Mutex<Box<dyn serialport::SerialPort>>,
    timeout: Mutex<Duration>,
}

impl SerialTransport {
    pub fn open(port_name: &str, baud: u32, timeout: Duration) -> io::Result<Self> {
        tracing::debug!(
            "Opening serial port {} at {} baud (timeout={:?})",
            port_name,
            baud,
            timeout
        );
        let port = serialport::new(port_name, baud)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::Two)
            .parity(serialport::Parity::None)
            .flow_control(serialport::FlowControl::None)
            .timeout(timeout)
            .open()
            .map_err(|e| {
                tracing::debug!("Failed to open serial port {}: {}", port_name, e);
                io::Error::other(e.to_string())
            })?;
        tracing::info!("Serial port {} opened (115200, 8N2, no flow control)", port_name);
        Ok(Self {
            port: Mutex::new(port),
            timeout: Mutex::new(timeout),
        })
    }

    pub fn flush_buffers(&self) -> io::Result<()> {
        let port = self.port.lock().map_err(|e| io::Error::other(e.to_string()))?;
        let _ = port.clear(serialport::ClearBuffer::All);
        tracing::debug!("Serial buffers cleared");
        Ok(())
    }

    pub fn into_inner(self) -> Box<dyn serialport::SerialPort> {
        self.port.into_inner().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl Transport for SerialTransport {
    async fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut port = self.port.lock().map_err(|e| io::Error::other(e.to_string()))?;
        Write::write_all(&mut *port, buf)?;
        #[cfg(feature = "trace-transport")]
        tracing::trace!(len = buf.len(), data = ?buf, "TX");
        Ok(())
    }

    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut port = self.port.lock().map_err(|e| io::Error::other(e.to_string()))?;
        let n = Read::read(&mut *port, buf)?;
        #[cfg(feature = "trace-transport")]
        tracing::trace!(len = n, data = ?&buf[..n], "RX");
        Ok(n)
    }

    async fn flush(&mut self) -> io::Result<()> {
        let mut port = self.port.lock().map_err(|e| io::Error::other(e.to_string()))?;
        Write::flush(&mut *port)
    }

    fn set_timeout(&mut self, timeout: Duration) {
        if let Ok(mut t) = self.timeout.lock() {
            *t = timeout;
        }
        if let Ok(mut port) = self.port.lock() {
            let _ = port.set_timeout(timeout);
        }
    }

    fn timeout(&self) -> Duration {
        self.timeout.lock().map(|t| *t).unwrap_or(Duration::from_secs(3))
    }
}
