use std::collections::BinaryHeap;
use std::io::{self, Read, Write};

#[derive(Debug, Clone)]
enum HuffmanNode {
    Leaf(u8),
    Internal(Box<HuffmanNode>, Box<HuffmanNode>),
}

struct HeapItem {
    weight: usize,
    node: HuffmanNode,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl Eq for HeapItem {}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.weight.cmp(&self.weight)
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct BitWriter<W: Write> {
    inner: W,
    buffer: u64,
    bits_buffered: u8,
}

impl<W: Write> BitWriter<W> {
    pub fn new(inner: W) -> Self {
        BitWriter {
            inner,
            buffer: 0,
            bits_buffered: 0,
        }
    }

    pub fn write_bits(&mut self, value: u32, bits: u8) -> io::Result<()> {
        let mask = if bits == 32 {
            0xFFFFFFFF
        } else {
            (1 << bits) - 1
        };
        let value = (value & mask) as u64;

        self.buffer |= value << self.bits_buffered;
        self.bits_buffered += bits;

        while self.bits_buffered >= 8 {
            let byte = (self.buffer & 0xFF) as u8;
            self.inner.write_all(&[byte])?;
            self.buffer >>= 8;
            self.bits_buffered -= 8;
        }
        Ok(())
    }

    pub fn write_bit(&mut self, bit: bool) -> io::Result<()> {
        self.write_bits(if bit { 1 } else { 0 }, 1)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        for &byte in bytes {
            self.write_bits(byte as u32, 8)?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        if self.bits_buffered > 0 {
            let byte = (self.buffer & 0xFF) as u8;
            self.inner.write_all(&[byte])?;
            self.buffer = 0;
            self.bits_buffered = 0;
        }
        self.inner.flush()
    }
}

pub struct BitReader<R: Read> {
    inner: R,
    buffer: u64,
    bits_buffered: u8,
}

impl<R: Read> BitReader<R> {
    pub fn new(inner: R) -> Self {
        BitReader {
            inner,
            buffer: 0,
            bits_buffered: 0,
        }
    }

    pub fn read_bit(&mut self) -> io::Result<bool> {
        if self.bits_buffered == 0 {
            let mut byte = [0u8; 1];
            self.inner.read_exact(&mut byte)?;
            self.buffer = byte[0] as u64;
            self.bits_buffered = 8;
        }
        let bit = (self.buffer & 1) != 0;
        self.buffer >>= 1;
        self.bits_buffered -= 1;
        Ok(bit)
    }

    pub fn read_bytes(&mut self, bytes: &mut [u8]) -> io::Result<()> {
        for byte in bytes.iter_mut() {
            let mut val = 0u8;
            for i in 0..8 {
                if self.read_bit()? {
                    val |= 1 << i;
                }
            }
            *byte = val;
        }
        Ok(())
    }
}

fn build_tree(frequencies: &[usize; 256]) -> Option<HuffmanNode> {
    let mut heap = BinaryHeap::new();
    for (symbol, &freq) in frequencies.iter().enumerate() {
        if freq > 0 {
            heap.push(HeapItem {
                weight: freq,
                node: HuffmanNode::Leaf(symbol as u8),
            });
        }
    }

    if heap.is_empty() {
        return None;
    }

    // Edge case: input only contains 1 unique byte pattern
    if heap.len() == 1 {
        let single = heap.pop().unwrap();
        let parent = HuffmanNode::Internal(
            Box::new(single.node),
            Box::new(HuffmanNode::Leaf(0)), // dummy node
        );
        return Some(parent);
    }

    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();
        let parent = HuffmanNode::Internal(Box::new(left.node), Box::new(right.node));
        heap.push(HeapItem {
            weight: left.weight + right.weight,
            node: parent,
        });
    }

    Some(heap.pop().unwrap().node)
}

fn serialize_tree<W: Write>(node: &HuffmanNode, writer: &mut BitWriter<W>) -> io::Result<()> {
    match node {
        HuffmanNode::Leaf(sym) => {
            writer.write_bit(true)?;
            writer.write_bytes(&[*sym])?;
        }
        HuffmanNode::Internal(left, right) => {
            writer.write_bit(false)?;
            serialize_tree(left, writer)?;
            serialize_tree(right, writer)?;
        }
    }
    Ok(())
}

fn deserialize_tree<R: Read>(reader: &mut BitReader<R>) -> io::Result<HuffmanNode> {
    let is_leaf = reader.read_bit()?;
    if is_leaf {
        let mut byte = [0u8; 1];
        reader.read_bytes(&mut byte)?;
        Ok(HuffmanNode::Leaf(byte[0]))
    } else {
        let left = deserialize_tree(reader)?;
        let right = deserialize_tree(reader)?;
        Ok(HuffmanNode::Internal(Box::new(left), Box::new(right)))
    }
}

fn generate_codes(node: &HuffmanNode, current_code: u32, depth: u8, table: &mut [(u32, u8); 256]) {
    match node {
        HuffmanNode::Leaf(sym) => {
            table[*sym as usize] = (current_code, depth);
        }
        HuffmanNode::Internal(left, right) => {
            generate_codes(left, current_code, depth + 1, table);
            generate_codes(right, current_code | (1 << depth), depth + 1, table);
        }
    }
}

pub fn encode<W: Write>(data: &[u8], writer: W) -> io::Result<usize> {
    let mut frequencies = [0usize; 256];
    for &byte in data {
        frequencies[byte as usize] += 1;
    }

    let tree = match build_tree(&frequencies) {
        Some(t) => t,
        None => return Ok(0),
    };

    let mut out_buf = Vec::new();
    {
        let mut bit_writer = BitWriter::new(&mut out_buf);
        serialize_tree(&tree, &mut bit_writer)?;

        let mut table = [(0u32, 0u8); 256];
        generate_codes(&tree, 0, 0, &mut table);

        for &byte in data {
            let (code, bits) = table[byte as usize];
            bit_writer.write_bits(code, bits)?;
        }
        bit_writer.flush()?;
    }

    let mut writer = writer;
    writer.write_all(&out_buf)?;
    Ok(out_buf.len())
}

pub fn decode<R: Read, W: Write>(reader: R, mut writer: W, expected_size: usize) -> io::Result<()> {
    if expected_size == 0 {
        return Ok(());
    }

    let mut bit_reader = BitReader::new(reader);
    let tree = deserialize_tree(&mut bit_reader)?;

    let mut decoded_bytes = 0;
    while decoded_bytes < expected_size {
        let mut current_node = &tree;
        loop {
            match current_node {
                HuffmanNode::Leaf(sym) => {
                    writer.write_all(&[*sym])?;
                    decoded_bytes += 1;
                    break;
                }
                HuffmanNode::Internal(left, right) => {
                    let bit = bit_reader.read_bit()?;
                    if bit {
                        current_node = right;
                    } else {
                        current_node = left;
                    }
                }
            }
        }
    }
    Ok(())
}
