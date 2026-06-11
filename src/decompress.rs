use crate::header::FileHeader;
use crc32fast::Hasher;
use rayon::prelude::*;
use std::io::{self, Read, Write};

fn decompress_single_block(comp_block: &[u8]) -> io::Result<Vec<u8>> {
    if comp_block.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Corrupt block payload: too short",
        ));
    }

    let uncompressed_len = u32::from_be_bytes(comp_block[0..4].try_into().unwrap()) as usize;
    let lz77_len = u32::from_be_bytes(comp_block[4..8].try_into().unwrap()) as usize;

    let mut lz77_buf = Vec::with_capacity(lz77_len);
    crate::huffman::decode(&comp_block[8..], &mut lz77_buf, lz77_len)?;

    let mut decompressed = Vec::with_capacity(uncompressed_len);
    let mut lz77_reader = std::io::Cursor::new(lz77_buf);

    while decompressed.len() < uncompressed_len {
        let mut flags_byte = [0u8; 1];
        lz77_reader.read_exact(&mut flags_byte)?;
        let flags = flags_byte[0];

        for i in 0..8 {
            if decompressed.len() >= uncompressed_len {
                break;
            }

            let is_match = (flags & (1 << i)) != 0;

            if is_match {
                let mut dist_bytes = [0u8; 2];
                lz77_reader.read_exact(&mut dist_bytes)?;
                let distance = u16::from_be_bytes(dist_bytes) as usize;

                let mut len_byte = [0u8; 1];
                lz77_reader.read_exact(&mut len_byte)?;
                let length = len_byte[0] as usize;

                if distance == 0 || distance > decompressed.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Corrupt stream: match distance {} is out of bounds",
                            distance
                        ),
                    ));
                }

                let start_idx = decompressed.len() - distance;
                for j in 0..length {
                    let byte = decompressed[start_idx + j];
                    decompressed.push(byte);
                }
            } else {
                let mut lit_byte = [0u8; 1];
                lz77_reader.read_exact(&mut lit_byte)?;
                decompressed.push(lit_byte[0]);
            }
        }
    }

    Ok(decompressed)
}

pub fn decompress<R: Read, W: Write>(mut reader: R, mut writer: W) -> io::Result<()> {
    let header = FileHeader::from_reader(&mut reader)?;

    let mut num_blocks_bytes = [0u8; 4];
    reader.read_exact(&mut num_blocks_bytes)?;
    let num_blocks = u32::from_be_bytes(num_blocks_bytes);

    let mut block_index = Vec::with_capacity(num_blocks as usize);
    for _ in 0..num_blocks {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        block_index.push(u32::from_be_bytes(size_bytes));
    }

    let mut compressed_blocks = Vec::with_capacity(num_blocks as usize);
    for size in block_index {
        let mut block_buf = vec![0u8; size as usize];
        reader.read_exact(&mut block_buf)?;
        compressed_blocks.push(block_buf);
    }

    let decompressed_blocks: io::Result<Vec<Vec<u8>>> = compressed_blocks
        .into_par_iter()
        .map(|comp_block| decompress_single_block(&comp_block))
        .collect();

    let decompressed_blocks = decompressed_blocks?;

    let mut final_decompressed = Vec::with_capacity(header.original_size as usize);
    for mut block in decompressed_blocks {
        final_decompressed.append(&mut block);
    }

    let mut hasher = Hasher::new();
    hasher.update(&final_decompressed);
    let calculated_checksum = hasher.finalize();

    if calculated_checksum != header.checksum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Checksum verification failed: Data corrupted",
        ));
    }

    writer.write_all(&final_decompressed)?;
    Ok(())
}
