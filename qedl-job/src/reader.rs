use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

/// Streaming file reader that reads chunks on demand
/// to avoid loading entire files into memory.
pub struct ChunkedReader {
    reader: BufReader<File>,
    chunk_size: usize,
    total_size: u64,
    bytes_read: u64,
}

impl ChunkedReader {
    pub fn new(path: &Path, chunk_size: usize) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let total_size = file.metadata()?.len();
        let reader = BufReader::with_capacity(chunk_size.min(64 * 1024), file);

        Ok(Self {
            reader,
            chunk_size,
            total_size,
            bytes_read: 0,
        })
    }

    /// Read the next chunk into the provided buffer.
    /// Returns the number of bytes read (0 = EOF).
    pub fn read_chunk(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let to_read = buf.len().min(self.chunk_size);
        let n = self.reader.read(&mut buf[..to_read])?;
        self.bytes_read += n as u64;
        Ok(n)
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    pub fn has_more(&self) -> bool {
        self.bytes_read < self.total_size
    }

    pub fn reset(&mut self) -> std::io::Result<()> {
        self.reader.seek(std::io::SeekFrom::Start(0))?;
        self.bytes_read = 0;
        Ok(())
    }
}
