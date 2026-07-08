# truv_compression

This project is a lossless compression program which uses a DEFLATE-inspired compression algorithm. By first sliding-window match-reduction (LZ77) then statistical entropy coding (Adaptive Huffman), the program achieves significant compression ratios while still maximizing throughput through data parallelism and hardware-level bit manipulation.

## Features

*   **Two-Pass Architecture:** Decouples LZ77 from Adaptive Huffman.
*   **Block-Parallel Concurrency:** Splits input files into independent 128 KB blocks and processes them concurrently across CPU cores using a work-stealing thread pool (`rayon`).
*   **SWAR SIMD Matching:** Replaces slow byte-by-byte string comparisons with 64-bit word comparisons; resolves  mismatch locations using bitwise XOR and trailing-zero calculations.
*   **Data Integrity:** Includes pre-order tree serialization, strict bounds audits to prevent relative match-offset exploits, and a CRC32 checksum verification.

---

## File Format

A compressed `.truv` file is packed as follows:

```text
+--------------------+----------------+------------+
| Global File Header | Block Metadata | Block Data |
+--------------------+----------------+------------+
```

### 1. Global File Header
*   **Magic Bytes (4 bytes):** `TRUV` (ASCII representation)
*   **Version (2 bytes):** `u16` format version
*   **Original File Size (8 bytes):** `u64` uncompressed file byte size
*   **CRC32 Checksum (4 bytes):** `u32` hash of the uncompressed data

### 2. Block Metadata
*   **Number of Blocks (4 bytes):** `u32` count of total compressed blocks
*   **Block Index Table (`N * 4` bytes):** An array of `u32` values, where each element represents the exact byte size of a compressed block payload.

### 3. Block Data
Each compressed block is independent and self-contained:
*   **Uncompressed Block Size (4 bytes):** `u32` original size
*   **LZ77 Stream Size (4 bytes):** `u32` size of the intermediate stream
*   **Serialized Huffman Tree:** Pre-order traversal representation of the local tree
*   **Huffman Bitstream:** The compressed payload bits

---

## Performance & Microbenchmarks

Testing on a repetitive ~270 KB text dataset yielded the following execution times:

| Phase | Architecture | Match Search Loop | Compression Time | Relative Latency |
| :--- | :--- | :--- | :--- | :--- |
| **v1** | Sequential | Byte-by-Byte | ~4.40 ms | 100% |
| **v2** | Block-Parallel (Multi-threaded) | Byte-by-Byte | ~2.42 ms | ~55% |
| **v3** | Block-Parallel (Multi-threaded) | **SWAR SIMD (64-bit word)** | **~1.08 ms** | **~24%** |

*Hardware-level bit scanning (`tzcnt`/`bsf`) allowed the SWAR matching logic to yield an additional ~57% speedup over the parallel-only baseline.*

---

## Usage

### Build
Compile the release binary:
```bash
cargo build --release
```

### Compress
Compress any file into the `.truv` format:
```bash
./target/release/truv_compression compress -i input.txt -o archive.truv
```

### Decompress
Restore the original file from the `.truv` archive:
```bash
./target/release/truv_compression decompress -i archive.truv -o output.txt
```

### Run Benchmarks
Run microbenchmarks locally using Criterion:
```bash
cargo bench
```
Open `target/criterion/report/index.html` in your browser to view the performance and statistical variance reports.
