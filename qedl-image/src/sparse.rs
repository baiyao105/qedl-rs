//! sparse image 解包
//! Magic: 0xED26FF3A

use crate::error::{ImageError, Result};
use bytes::Bytes;
use std::io::{Read, Seek, SeekFrom};

pub const SPARSE_HEADER_MAGIC: u32 = 0xED26FF3A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ChunkType {
    Raw = 0xCAC1,
    Fill = 0xCAC2,
    DontCare = 0xCAC3,
    Crc32 = 0xCAC4,
}

impl ChunkType {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            0xCAC1 => Some(Self::Raw),
            0xCAC2 => Some(Self::Fill),
            0xCAC3 => Some(Self::DontCare),
            0xCAC4 => Some(Self::Crc32),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SparseHeader {
    pub magic: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub file_hdr_sz: u16,
    pub chunk_hdr_sz: u16,
    pub blk_sz: u32,
    pub total_blks: u32,
    pub total_chunks: u32,
    pub image_checksum: u32,
}

impl SparseHeader {
    pub const SIZE: usize = 28;

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ImageError::SparseFailed {
                reason: "header too short".to_string(),
            });
        }

        let magic = u32::from_le_bytes(data[0..4].try_into().map_err(|e| ImageError::SparseFailed {
            reason: format!("failed to parse magic: {}", e),
        })?);
        if magic != SPARSE_HEADER_MAGIC {
            return Err(ImageError::SparseFailed {
                reason: format!("invalid magic: 0x{:08X}", magic),
            });
        }

        Ok(Self {
            magic,
            major_version: u16::from_le_bytes(data[4..6].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse major_version: {}", e),
            })?),
            minor_version: u16::from_le_bytes(data[6..8].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse minor_version: {}", e),
            })?),
            file_hdr_sz: u16::from_le_bytes(data[8..10].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse file_hdr_sz: {}", e),
            })?),
            chunk_hdr_sz: u16::from_le_bytes(data[10..12].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse chunk_hdr_sz: {}", e),
            })?),
            blk_sz: u32::from_le_bytes(data[12..16].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse blk_sz: {}", e),
            })?),
            total_blks: u32::from_le_bytes(data[16..20].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse total_blks: {}", e),
            })?),
            total_chunks: u32::from_le_bytes(data[20..24].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse total_chunks: {}", e),
            })?),
            image_checksum: u32::from_le_bytes(data[24..28].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse image_checksum: {}", e),
            })?),
        })
    }

    pub fn is_sparse(data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&data[0..4]);
        u32::from_le_bytes(buf) == SPARSE_HEADER_MAGIC
    }
}

#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub chunk_type: ChunkType,
    pub chunk_data_sz: u32,
    pub total_sz: u32,
}

impl ChunkHeader {
    pub const SIZE: usize = 12;

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ImageError::SparseFailed {
                reason: "chunk header too short".to_string(),
            });
        }

        let chunk_type_val = u16::from_le_bytes(data[0..2].try_into().map_err(|e| ImageError::SparseFailed {
            reason: format!("failed to parse chunk_type: {}", e),
        })?);
        let chunk_type = ChunkType::from_u16(chunk_type_val).ok_or_else(|| ImageError::SparseFailed {
            reason: format!("unknown chunk type: 0x{:04X}", chunk_type_val),
        })?;

        Ok(Self {
            chunk_type,
            chunk_data_sz: u32::from_le_bytes(data[4..8].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse chunk_data_sz: {}", e),
            })?),
            total_sz: u32::from_le_bytes(data[8..12].try_into().map_err(|e| ImageError::SparseFailed {
                reason: format!("failed to parse total_sz: {}", e),
            })?),
        })
    }
}

pub struct SparseExpander<R: Read + Seek> {
    reader: R,
    header: SparseHeader,
}

impl<R: Read + Seek> SparseExpander<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let mut hdr_buf = [0u8; SparseHeader::SIZE];
        reader.read_exact(&mut hdr_buf).map_err(|e| ImageError::SparseFailed {
            reason: format!("failed to read sparse header: {}", e),
        })?;
        let header = SparseHeader::from_bytes(&hdr_buf)?;

        let extra = header.file_hdr_sz as usize - SparseHeader::SIZE;
        if extra > 0 {
            reader
                .seek(SeekFrom::Current(extra as i64))
                .map_err(|e| ImageError::SparseFailed {
                    reason: format!("seek error: {}", e),
                })?;
        }

        Ok(Self { reader, header })
    }

    pub fn raw_size(&self) -> u64 {
        self.header.total_blks as u64 * self.header.blk_sz as u64
    }

    pub fn header(&self) -> &SparseHeader {
        &self.header
    }

    /// 逐 chunk 遍历，对每个 chunk 调用回调
    ///
    /// 回调参数: (chunk_type, data)
    pub fn for_each_chunk<F>(&mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(ChunkType, &[u8]) -> Result<()>,
    {
        let chunk_hdr_sz = self.header.chunk_hdr_sz as usize;
        let blk_sz = self.header.blk_sz as usize;

        let mut hdr_buf = vec![0u8; chunk_hdr_sz];
        let mut data_buf = Vec::new();
        let zeros = vec![0u8; blk_sz];

        loop {
            match self.reader.read_exact(&mut hdr_buf) {
                Ok(()) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    return Err(ImageError::SparseFailed {
                        reason: format!("failed to read chunk header: {}", e),
                    });
                }
            }

            let chunk = ChunkHeader::from_bytes(&hdr_buf)?;

            match chunk.chunk_type {
                ChunkType::Raw => {
                    let data_size = chunk.chunk_data_sz as usize;
                    if data_buf.len() < data_size {
                        data_buf.resize(data_size, 0);
                    }
                    self.reader
                        .read_exact(&mut data_buf[..data_size])
                        .map_err(|e| ImageError::SparseFailed {
                            reason: format!("failed to read raw chunk: {}", e),
                        })?;
                    callback(ChunkType::Raw, &data_buf[..data_size])?;
                }
                ChunkType::Fill => {
                    let mut fill_buf = [0u8; 4];
                    self.reader
                        .read_exact(&mut fill_buf)
                        .map_err(|e| ImageError::SparseFailed {
                            reason: format!("failed to read fill value: {}", e),
                        })?;
                    let num_blks = chunk.chunk_data_sz as usize / 4;
                    for _ in 0..num_blks {
                        callback(ChunkType::Fill, &fill_buf)?;
                    }
                }
                ChunkType::DontCare => {
                    let skip_size = chunk.chunk_data_sz as usize;
                    let mut skipped = 0;
                    while skipped < skip_size {
                        let to_read = (skip_size - skipped).min(blk_sz);
                        callback(ChunkType::DontCare, &zeros[..to_read])?;
                        skipped += to_read;
                    }
                }
                ChunkType::Crc32 => {
                    let skip = chunk.chunk_data_sz as usize;
                    self.reader
                        .seek(SeekFrom::Current(skip as i64))
                        .map_err(|e| ImageError::SparseFailed {
                            reason: format!("seek error on crc chunk: {}", e),
                        })?;
                }
            }
        }

        Ok(())
    }
}

pub fn expand_to_vec(data: &[u8]) -> Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(data);
    let mut expander = SparseExpander::new(cursor)?;
    let blk_sz = expander.header().blk_sz as usize;
    let mut output = Vec::with_capacity(expander.raw_size() as usize);
    let mut fill_block = vec![0u8; blk_sz];

    expander.for_each_chunk(|chunk_type, chunk_data| {
        match chunk_type {
            ChunkType::Fill => {
                let fill = &chunk_data[..4.min(chunk_data.len())];
                for chunk in fill_block.chunks_exact_mut(4) {
                    chunk.copy_from_slice(fill);
                }
                output.extend_from_slice(&fill_block);
            }
            _ => {
                output.extend_from_slice(chunk_data);
            }
        }
        Ok(())
    })?;

    Ok(output)
}

pub fn expand_to_bytes(data: &[u8]) -> Result<Bytes> {
    Ok(Bytes::from(expand_to_vec(data)?))
}
