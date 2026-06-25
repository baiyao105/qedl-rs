use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::io;
use std::time::Duration;

/// Async transport abstraction for device communication.
///
/// Implementations must be Send + Sync to allow concurrent operations.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    async fn write(&mut self, buf: &[u8]) -> io::Result<()>;

    async fn write_bytes(&mut self, buf: Bytes) -> io::Result<()> {
        self.write(&buf).await
    }

    async fn read_exact_bytes(&mut self, len: usize) -> io::Result<Bytes> {
        let mut buf = BytesMut::with_capacity(len);
        buf.resize(len, 0);
        self.read_exact(&mut buf).await?;
        Ok(buf.freeze())
    }

    async fn flush(&mut self) -> io::Result<()>;

    async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut pos = 0;
        while pos < buf.len() {
            let n = self.read(&mut buf[pos..]).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to fill whole buffer",
                ));
            }
            pos += n;
        }
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration);

    fn timeout(&self) -> Duration;
}

#[async_trait]
impl Transport for Box<dyn Transport> {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf).await
    }

    async fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write(buf).await
    }

    async fn write_bytes(&mut self, buf: bytes::Bytes) -> io::Result<()> {
        (**self).write_bytes(buf).await
    }

    async fn read_exact_bytes(&mut self, len: usize) -> io::Result<bytes::Bytes> {
        (**self).read_exact_bytes(len).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        (**self).flush().await
    }

    fn set_timeout(&mut self, timeout: Duration) {
        (**self).set_timeout(timeout)
    }

    fn timeout(&self) -> Duration {
        (**self).timeout()
    }
}
