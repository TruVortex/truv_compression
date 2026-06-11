use crate::header::FileHeader;
use crc32fast::Hasher;
use rayon::prelude::*;
use std::io::{self, Write};

const BLOCK_SIZE: usize = 128 * 1024;
const WINDOW_SIZE: usize = 65535;
const MIN_MATCH_LEN: usize = 3;
const MAX_MATCH_LEN: usize = 255;
const MAX_CHAIN_DEPTH: usize = 32;

enum Token {
    Literal(u8),
    Match { distance: u16, length: u8 },
}

fn get_hash(bytes: &[u8]) -> usize {
    let val = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
    ((val.wrapping_mul(2654435761) >> 16) & 0xFFFF) as usize
}

fn write_block<W: Write>(writer: &mut W, tokens: &[Token]) -> io::Result<usize> {
    if tokens.is_empty() {
        return Ok(0);
    }

    let mut flags = 0u8;
    for (i, token) in tokens.iter().enumerate() {
        if let Token::Match { .. } = token {
            flags |= 1 << i;
        }
    }

    let mut written = 0;
    writer.write_all(&[flags])?;
    written += 1;

    for token in tokens {
        match token {
            Token::Literal(b) => {
                writer.write_all(&[*b])?;
                written += 1;
            }
            Token::Match { distance, length } => {
                writer.write_all(&distance.to_be_bytes())?;
                writer.write_all(&[*length])?;
                written += 3;
            }
        }
    }

    Ok(written)
}

fn compress_single_block(chunk: &[u8]) -> io::Result<Vec<u8>> {
    let mut lz77_buf = Vec::new();
    let mut head = vec![-1i32; 65536];
    let mut prev = vec![-1i32; chunk.len()];
    let mut pos = 0;
    let mut token_buffer = Vec::with_capacity(8);

    while pos < chunk.len() {
        let mut best_len = 0;
        let mut best_dist = 0;

        if pos + MIN_MATCH_LEN <= chunk.len() {
            let hash = get_hash(&chunk[pos..pos + MIN_MATCH_LEN]);
            let mut match_pos = head[hash];

            let mut depth = 0;
            while match_pos != -1 && depth < MAX_CHAIN_DEPTH {
                let dist = pos - match_pos as usize;
                if dist > WINDOW_SIZE {
                    break;
                }

                let mut len = 0;

                while pos + len + 8 <= chunk.len()
                    && match_pos as usize + len + 8 <= pos
                    && len < MAX_MATCH_LEN - 8
                {
                    let val_pos =
                        u64::from_le_bytes(chunk[pos + len..pos + len + 8].try_into().unwrap());
                    let val_match = u64::from_le_bytes(
                        chunk[match_pos as usize + len..match_pos as usize + len + 8]
                            .try_into()
                            .unwrap(),
                    );

                    if val_pos == val_match {
                        len += 8;
                    } else {
                        let diff = val_pos ^ val_match;
                        let matching_bytes = (diff.trailing_zeros() / 8) as usize;
                        len += matching_bytes;
                        break;
                    }
                }

                while pos + len < chunk.len()
                    && match_pos as usize + len < pos
                    && len < MAX_MATCH_LEN
                {
                    if chunk[pos + len] == chunk[match_pos as usize + len] {
                        len += 1;
                    } else {
                        break;
                    }
                }
                // ---------------------------------------------------------

                if len >= MIN_MATCH_LEN && len > best_len {
                    best_len = len;
                    best_dist = dist;
                    if len == MAX_MATCH_LEN {
                        break;
                    }
                }

                match_pos = prev[match_pos as usize];
                depth += 1;
            }
        }

        if best_len >= MIN_MATCH_LEN {
            token_buffer.push(Token::Match {
                distance: best_dist as u16,
                length: best_len as u8,
            });

            for offset in 0..best_len {
                let insert_pos = pos + offset;
                if insert_pos + MIN_MATCH_LEN <= chunk.len() {
                    let hash = get_hash(&chunk[insert_pos..insert_pos + MIN_MATCH_LEN]);
                    prev[insert_pos] = head[hash];
                    head[hash] = insert_pos as i32;
                }
            }
            pos += best_len;
        } else {
            token_buffer.push(Token::Literal(chunk[pos]));

            if pos + MIN_MATCH_LEN <= chunk.len() {
                let hash = get_hash(&chunk[pos..pos + MIN_MATCH_LEN]);
                prev[pos] = head[hash];
                head[hash] = pos as i32;
            }
            pos += 1;
        }

        if token_buffer.len() == 8 {
            write_block(&mut lz77_buf, &token_buffer)?;
            token_buffer.clear();
        }
    }

    if !token_buffer.is_empty() {
        write_block(&mut lz77_buf, &token_buffer)?;
    }

    let mut block_payload = Vec::new();
    block_payload.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
    block_payload.extend_from_slice(&(lz77_buf.len() as u32).to_be_bytes());

    crate::huffman::encode(&lz77_buf, &mut block_payload)?;

    Ok(block_payload)
}

pub fn compress<W: Write>(input: &[u8], mut writer: W) -> io::Result<usize> {
    let original_size = input.len() as u64;

    let mut hasher = Hasher::new();
    hasher.update(input);
    let checksum = hasher.finalize();

    let chunks: Vec<&[u8]> = input.chunks(BLOCK_SIZE).collect();
    let compressed_blocks: io::Result<Vec<Vec<u8>>> = chunks
        .into_par_iter()
        .map(|chunk| compress_single_block(chunk))
        .collect();

    let compressed_blocks = compressed_blocks?;

    let header = FileHeader {
        version: 1,
        original_size,
        checksum,
    };
    writer.write_all(&header.to_bytes())?;
    let mut bytes_written = header.to_bytes().len();

    let num_blocks = compressed_blocks.len() as u32;
    writer.write_all(&num_blocks.to_be_bytes())?;
    bytes_written += 4;

    for block in &compressed_blocks {
        let size = block.len() as u32;
        writer.write_all(&size.to_be_bytes())?;
        bytes_written += 4;
    }

    for block in compressed_blocks {
        writer.write_all(&block)?;
        bytes_written += block.len();
    }

    Ok(bytes_written)
}
