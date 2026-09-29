# Phase 1 Context: Core Chunking & Streaming Engine

## Purpose & Scope
This phase implements the primary chunking and packaging engine in Rust. It takes a folder or file of any size (e.g. 40GB) and slices it into bounded chunk files (e.g. 14GB each for a 15GB pen drive) with a complete manifest and cryptographic checksums.

## User Decisions & Constraints
- **Primary Speed Goal**: Reconstruction must happen in "a matter of seconds". Therefore:
  - Default mode provides ultra-fast streaming (Store mode with 0 CPU overhead, or Zstandard at Level 1).
  - Heavy, slow compression algorithms (like LZMA/7z maximum) are avoided.
- **Handling Large Single Files**:
  - Individual files larger than the chunk size (e.g., a 25GB file into 14GB chunks) are sliced seamlessly across part boundaries using a streaming archive abstraction (TAR stream sliced at chunk boundaries).
- **Target OS & Cross-Platform**:
  - Fully cross-platform (Linux, Windows, macOS).
  - Implementation in Rust for single static binary portability and high I/O throughput.
- **Workflow**:
  - Offline batch splitting: The tool writes all part files and helper scripts into a target folder or directly to specified destination, allowing the user to shuttle parts manually via pen drive.

## Implementation Architecture
1. **CLI Commands**:
   - `split <SOURCE_PATH> -o <OUTPUT_DIR> -s <SIZE>`
     - Flags:
       - `-s, --chunk-size <SIZE>`: Parse human-friendly sizes (e.g., `14G`, `14GB`, `4000M`, `500MB`).
       - `-m, --mode <store|fast>`: `store` (uncompressed raw stream, max disk speed) or `fast` (Zstd level 1).
       - `-o, --output <DIR>`: Output directory for chunks and manifest.
2. **Chunk Output Format**:
   - Files named: `data.part001`, `data.part002`, `data.part003`, ...
   - `manifest.json`:
     - Original source path and name
     - Total uncompressed size and file count
     - Mode (`store` or `zstd`)
     - Chunk size and list of parts (filename, byte size, SHA-256 hash)
3. **Chunking Engine Design**:
   - Uses a custom `ChunkedWriter` that implements `std::io::Write`.
   - When the byte count of current chunk reaches the chunk limit, closes current file and opens `data.part{n+1}`.
   - Calculates running SHA-256 checksums per chunk on the fly without second passes.
   - Low memory footprint: Fixed streaming buffer (e.g. 1MB - 8MB buffer), O(1) memory usage.

## Deliverables
- Fully functional Rust CLI crate `fastchunk` (or `compressor`).
- Unit and integration tests for chunking single large files and nested folders.
- Clean manifest generation.
