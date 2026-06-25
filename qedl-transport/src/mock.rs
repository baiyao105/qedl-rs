use crate::port::Transport;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::VecDeque;
use std::io;
use std::time::Duration;

pub struct MockTransport {
    read_queue: VecDeque<Bytes>,
    write_log: Vec<Bytes>,

    read_delay: Option<Duration>,
    write_delay: Option<Duration>,

    read_error: Option<io::ErrorKind>,
    write_error: Option<io::ErrorKind>,
    error_after_reads: Option<usize>,
    error_after_writes: Option<usize>,

    disconnect_after_reads: Option<usize>,
    disconnect_after_writes: Option<usize>,

    corrupt_after_reads: Option<usize>,
    corrupt_byte: u8,

    read_count: usize,
    write_count: usize,
    timeout: Duration,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            read_queue: VecDeque::new(),
            write_log: Vec::new(),
            read_delay: None,
            write_delay: None,
            read_error: None,
            write_error: None,
            error_after_reads: None,
            error_after_writes: None,
            disconnect_after_reads: None,
            disconnect_after_writes: None,
            corrupt_after_reads: None,
            corrupt_byte: 0xFF,
            read_count: 0,
            write_count: 0,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn push_read_data(&mut self, data: &[u8]) {
        self.read_queue.push_back(Bytes::copy_from_slice(data));
    }

    pub fn push_read(&mut self, data: Bytes) {
        self.read_queue.push_back(data);
    }

    pub fn push_reads(&mut self, data: Vec<Bytes>) {
        for d in data {
            self.push_read(d);
        }
    }

    pub fn written_data(&self) -> Vec<&Bytes> {
        self.write_log.iter().collect()
    }

    pub fn bytes_written(&self) -> usize {
        self.write_log.iter().map(|b| b.len()).sum()
    }

    pub fn clear_write_log(&mut self) {
        self.write_log.clear();
    }

    pub fn with_read_delay(mut self, delay: Duration) -> Self {
        self.read_delay = Some(delay);
        self
    }

    pub fn with_write_delay(mut self, delay: Duration) -> Self {
        self.write_delay = Some(delay);
        self
    }

    pub fn with_read_error_after(mut self, after: usize, kind: io::ErrorKind) -> Self {
        self.error_after_reads = Some(after);
        self.read_error = Some(kind);
        self
    }

    pub fn with_write_error_after(mut self, after: usize, kind: io::ErrorKind) -> Self {
        self.error_after_writes = Some(after);
        self.write_error = Some(kind);
        self
    }

    pub fn with_disconnect_after_reads(mut self, after: usize) -> Self {
        self.disconnect_after_reads = Some(after);
        self
    }

    pub fn with_disconnect_after_writes(mut self, after: usize) -> Self {
        self.disconnect_after_writes = Some(after);
        self
    }

    pub fn with_corrupt_after_reads(mut self, after: usize, corrupt_byte: u8) -> Self {
        self.corrupt_after_reads = Some(after);
        self.corrupt_byte = corrupt_byte;
        self
    }

    pub fn reset(&mut self) {
        self.read_queue.clear();
        self.write_log.clear();
        self.read_count = 0;
        self.write_count = 0;
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(delay) = self.read_delay {
            tokio::time::sleep(delay).await;
        }

        self.read_count += 1;

        if let Some(max) = self.disconnect_after_reads
            && self.read_count > max
        {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "mock disconnect"));
        }

        if let (Some(kind), Some(max)) = (self.read_error, self.error_after_reads)
            && self.read_count > max
        {
            return Err(io::Error::new(kind, "mock read error"));
        }

        if let Some(data) = self.read_queue.pop_front() {
            let len = std::cmp::min(buf.len(), data.len());

            if let Some(corrupt_at) = self.corrupt_after_reads
                && self.read_count >= corrupt_at
            {
                buf[..len].copy_from_slice(&data[..len]);
                if len > 0 {
                    buf[0] = self.corrupt_byte;
                }
                return Ok(len);
            }

            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    async fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        if let Some(delay) = self.write_delay {
            tokio::time::sleep(delay).await;
        }

        self.write_count += 1;

        if let Some(max) = self.disconnect_after_writes
            && self.write_count > max
        {
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, "mock disconnect"));
        }

        if let (Some(kind), Some(max)) = (self.write_error, self.error_after_writes)
            && self.write_count > max
        {
            return Err(io::Error::new(kind, "mock write error"));
        }

        self.write_log.push(Bytes::copy_from_slice(buf));
        Ok(())
    }

    async fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }
}
