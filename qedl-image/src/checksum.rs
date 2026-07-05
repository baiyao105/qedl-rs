use crc32fast::Hasher;
use md5::Md5;
use sha2::Sha256;
use std::fs::File;
use std::io::{BufReader, Read, Result};
use std::path::Path;

pub fn compute_md5(data: &[u8]) -> String {
    use md5::Digest;
    let mut hasher = Md5::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hex::encode(hash)
}

pub fn compute_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hex::encode(hash)
}

pub fn compute_crc32(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

pub fn compute_file_md5(path: &Path) -> Result<String> {
    use md5::Digest;
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Md5::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

pub fn compute_file_sha256(path: &Path) -> Result<String> {
    use sha2::Digest;
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

pub fn compute_file_crc32(path: &Path) -> Result<u32> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Hasher::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}
