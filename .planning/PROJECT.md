# Project: FastChunk (Folder/File Sized Chunk & Fast Restore Tool)

## Overview
A high-performance file and folder chunking tool written in Rust designed for transferring large datasets (e.g., 40GB+) across capacity-constrained physical media (e.g., a 15GB flash drive) or size-limited transport layers.

The tool splits large files and folders into user-configurable part sizes (e.g. 14GB, 4GB, etc.), generates integrity checksums, and bundles a zero-dependency setup (portable shell/batch scripts and a standalone native binary) so the destination machine can reconstruct the original folder and files in a matter of seconds.

## Problem Statement
When transferring a large directory (e.g., 40GB) across smaller storage devices (e.g., a 15GB USB pen drive), the process breaks down if individual files exceed the capacity of the drive or if manual slicing is tedious. Conventional compression tools (like 7-Zip or XZ with high compression) take dozens of minutes to compress and decompress large datasets. Users need:
1. Slicing that works across large individual files and deep directory trees.
2. Extraction that happens in **seconds** (disk speed / multi-threaded Zstd or raw streaming).
3. Self-contained restoration at the target machine without installing specialized third-party software.
4. Part integrity checks and missing-part warnings before reconstruction.

## Key Requirements & Scope
- **Split & Chunking Engine**:
  - Accepts single files or recursive directories.
  - Slices into arbitrary part sizes (`--chunk-size 14G`, `4G`, `1000M`, etc.).
  - Handles single files larger than the chunk size by streaming archive chunking.
- **Speed & Extraction**:
  - High-speed mode using Zstandard (level 1 / multi-threaded) and Zero-compression (Store) mode for pure disk-speed throughput.
  - Intelligent skipping of already-compressed files (mp4, mkv, zip, etc.) to save CPU.
- **Reconstruction Setup**:
  - Auto-generates `restore.sh` (POSIX Linux/macOS) and `restore.bat` / `restore.ps1` (Windows native).
  - Bundles or compiles a standalone, portable native extractor executable (`restore`).
  - Verifies presence of all parts before starting reconstruction; clearly alerts which parts are missing.
- **Data Integrity**:
  - Manifest generation (`manifest.json`) storing part metadata, total byte size, file tree, and fast checksums (BLAKE3 or SHA-256).

## Technology Stack
- **Language**: Rust (static binary, zero runtime dependencies, high concurrency).
- **Compression**: `zstd` (fast streaming decompression at 1.5 - 2+ GB/s).
- **Archive Format**: Streaming TAR / custom chunk stream for zero-overhead reconstruction.
- **Platform**: Cross-platform (Linux, Windows, macOS).
