# truv_compression

This project is a lossless compression program which uses the DEFLATE compression algorithm. By decoupling sliding-window match-reduction (LZ77) from statistical entropy coding (Dynamic Huffman), the program achieves significant compression ratios while maximizing throughput through data parallelism and hardware-level bit manipulation.

---

## Key Features

*   **Two-Pass Architecture:** Decouples pattern-matching (LZ77) from bit-packing (Dynamic Huffman).
*   **Block-Parallel Concurrency:** Splits input files into independent 128 KB blocks and processes them concurrently across CPU cores using a work-stealing thread pool (`rayon`).
*   **SWAR SIMD Matching:** Replaces slow byte-by-byte string comparisons with 64-bit word comparisons. It resolves exact mismatch locations using bitwise XOR and hardware-level trailing-zero calculations.
*   **Data Integrity:** Includes pre-order tree serialization, strict bounds audits to prevent relative match-offset exploits, and a localized CRC32 checksum verification.

---

## Architecture & Data Flow

```text
                     [ Input File ]
                           │
              (Split into 128 KB Blocks)
                           │
       ┌───────────────────┼───────────────────┐
       ▼                   ▼                   ▼
   [Block 1]           [Block 2]           [Block 3]    ... (Rayon Thread Pool)
   ┌───┴───┐           ┌───┴───┐           ┌───┴───┐
   │ LZ77  │           │ LZ77  │           │ LZ77  │    (SWAR SIMD Matching)
   │Huffman│           │Huffman│           │Huffman│    (Dynamic Tree Serialization)
   └───┬───┘           └───┬───┘           └───┬───┘
       └───────────────────┼───────────────────┘
                           ▼
                  [ Ordered Assembly ] ──> Generates Block Index
                           │
                     [ .truv File ]
```

---

## The `.truv` Binary Layout

A compressed `.truv` file is packed as follows:

```text
+-------------------+-----------------+----------------------+
| Global File Header| Block Metadata  | Block Data Payloads  |
+-------------------+-----------------+----------------------+
```

### 1. Global File Header (18 bytes)
*   **Magic Bytes (4 bytes):** `TRUV` (ASCII representation)
*   **Version (2 bytes):** `u16` format version
*   **Original File Size (8 bytes):** `u64` uncompressed file byte size
*   **CRC32 Checksum (4 bytes):** `u32` hash of the uncompressed data

### 2. Block Metadata Index
*   **Number of Blocks (4 bytes):** `u32` count of total compressed blocks
*   **Block Index Table (`N * 4` bytes):** An array of `u32` values, where each element represents the exact byte size of a compressed block payload.

### 3. Block Data Payloads
Each compressed block is independent and self-contained:
*   **Uncompressed Block Size (4 bytes):** `u32` original size
*   **LZ77 Stream Size (4 bytes):** `u32` size of the intermediate stream
*   **Serialized Huffman Tree:** Pre-order traversal representation of the local tree
*   **Huffman Bitstream:** The compressed payload bits

---

## Performance & Microbenchmarks

Testing on a highly repetitive ~270 KB text dataset yielded the following execution times:

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
Open `target/criterion/report/index.html` in your browser to view the detailed performance and statistical variance reports.
