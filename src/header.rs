use std::io::{self, Read};

pub const MAGIC_BYTES: &[u8; 4] = b"TRUV";
pub const HEADER_SIZE: usize = 18;

#[derive(Debug, PartialEq)]
pub struct FileHeader {
    pub version: u16,
    pub original_size: u64,
    pub checksum: u32,
}

impl FileHeader {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE);
        bytes.extend_from_slice(MAGIC_BYTES);
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes.extend_from_slice(&self.original_size.to_be_bytes());
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes
    }

    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic bytes: Not a valid .truv file",
            ));
        }
        let mut version_bytes = [0u8; 2];
        reader.read_exact(&mut version_bytes)?;
        let version = u16::from_be_bytes(version_bytes);
        let mut size_bytes = [0u8; 8];
        reader.read_exact(&mut size_bytes)?;
        let original_size = u64::from_be_bytes(size_bytes);
        let mut checksum_bytes = [0u8; 4];
        reader.read_exact(&mut checksum_bytes)?;
        let checksum = u32::from_be_bytes(checksum_bytes);
        Ok(FileHeader {
            version,
            original_size,
            checksum,
        })
    }
}
