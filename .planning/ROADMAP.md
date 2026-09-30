# Roadmap

## Phase 1: Core Chunking & Streaming Engine [x] (completed 2026-09-30)
**Goal**: Build the core Rust CLI and streaming engine that packages files/folders, calculates checksums, and splits into fixed-size chunks (`part_001.bin`, etc.) while generating a robust `manifest.json`.
- [x] Initialize Cargo project with dependencies (`clap`, `zstd`, `sha2`, `serde`, `tar`, `indicatif`).
- [x] Implement chunk-size parser (`14G`, `500M`, `4000MB`).
- [x] Implement streaming archiver that writes directly into sliced chunk files of exact byte limits.
- [x] Implement manifest generator with file list, chunk metadata, and SHA-256 hashes.
- [x] Provide CLI command `split <SRC> -o <OUT> -s <SIZE> [--store | --fast]`.

## Phase 2: Zero-Dependency Setup & Fast Restorer [x] (completed 2026-09-30)
**Goal**: Build the destination reconstruction pipeline that extracts in seconds without external dependencies, detecting missing parts before execution.
- [x] Generate self-contained `restore.sh` (Linux/macOS) and `restore.bat`/`restore.ps1` (Windows native).
- [x] Implement native Rust `restore <PARTS_DIR> -o <TARGET_DIR>` subcommand.
- [x] Implement pre-flight part verification: fail early with clear warnings if parts (e.g. part 3 of 4) are missing.
- [x] Support fast multi-threaded Zstandard decompression and zero-compression stream reassembly.
- [x] Implement progress reporting during extraction.

## Phase 3: Validation, Large Dataset Benchmarking & Ergonomics
**Goal**: Verify performance on large datasets, validate cross-platform compatibility, and add UX polish.
- [ ] End-to-end integration tests simulating single-pen-drive multi-part transfers.
- [ ] Benchmark extraction speed on large files/directories (verifying seconds-scale extraction).
- [ ] Add standalone single-binary bundle support so the restorer binary can be copied directly alongside the parts.
- [ ] Documentation and user guide.
