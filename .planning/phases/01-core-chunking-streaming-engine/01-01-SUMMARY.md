# Phase 01: Core Chunking & Streaming Engine - Plan 01-01 Summary

## Overview

Plan 01-01 delivered the core Rust CLI and streaming engine for FastChunk. The engine packages single files or recursive directories of arbitrary size, calculates cryptographic checksums on the fly, slices the stream into exact byte-limited chunk files (`data.part001`, `data.part002`, ...), and generates a structured `manifest.json`.

## Deliverables & Key Artifacts

1. **Cargo Configuration & Dependencies (`Cargo.toml`)**:
   - Packaged as `fastchunk` (Rust 2021 edition).
   - Dependencies: `clap` (derive), `zstd` (with `zstdmt`), `sha2`, `hex`, `serde`, `serde_json`, `tar`, `parse-size`, `indicatif`, and `tempfile`.

2. **Human Size Parser (`src/size_parser.rs`)**:
   - `parse_human_size` parses human-friendly size representations (e.g. `14G`, `14GB`, `4000MB`, `500M`, `1GiB`).
   - Validates that chunk sizes meet a safe minimum threshold (64KB / 65,536 bytes) to prevent chunk thrashing.

3. **Streaming ChunkedWriter (`src/chunker.rs`)**:
   - Implements `std::io::Write` with exact byte boundary splitting into sequential files (`data.part001`, `data.part002`, etc.).
   - Computes SHA-256 digests on the fly with zero memory overhead ($O(1)$ memory usage).
   - Handles exact chunk multiple boundaries cleanly, preventing empty trailing parts while preserving single-part generation for empty input streams.
   - Exports `ChunkedWriter` and `PartInfo`.

4. **Streaming Archive Packager (`src/archiver.rs`)**:
   - `pack_archive` walks source directories or files and streams TAR entries directly into either raw store or multi-threaded Zstd level 1 compression.
   - Pipes directly into `ChunkedWriter` without intermediate temporary disk files.
   - Exports `pack_archive` and `PackagingResult`.

5. **Manifest Generation & CLI Wiring (`src/manifest.rs`, `src/cli.rs`, `src/main.rs`)**:
   - `Manifest` schema records source metadata, mode, total entries, raw uncompressed size, chunk size, and per-part details (`filename`, `size`, `sha256`).
   - CLI command `fastchunk split <SOURCE> -o <OUTPUT> -s <CHUNK_SIZE> [-m store|fast] [-v]` coordinates packaging, writes `manifest.json`, and displays completion summaries.

6. **Comprehensive Integration Test Suite (`tests/chunking_tests.rs`)**:
   - `test_split_single_large_file_across_chunks`: 15MB file split across 4MB chunks into 4 parts, verifying exact part sizes, manifest hash accuracy, and byte-for-byte stream reconstruction via TAR.
   - `test_split_nested_directory_fast_zstd`: Complex directory tree split with Zstd compression, verifying multi-part chunking and clean decompression/unpacking.
   - `test_exact_chunk_boundary_no_empty_trailing_part`: Stream landing on exact chunk boundary does not emit empty trailing parts.
   - `test_posix_pipeline_compatibility`: Verifies standard Unix pipeline restoration (`cat data.part* | tar -tf -` and `cat data.part* | zstd -d | tar -tf -`) with system tools.

## Verification Evidence

All automated unit and integration tests passed cleanly:
```text
running 10 tests
test size_parser::tests::test_underflow_and_invalid_inputs ... ok
test size_parser::tests::test_valid_units ... ok
test chunker::tests::test_empty_stream_produces_single_empty_part ... ok
test chunker::tests::test_exact_multiple_no_trailing_empty_part ... ok
test cli::tests::test_cli_default_mode_is_fast ... ok
test cli::tests::test_cli_parse_split ... ok
test chunker::tests::test_multipart_splitting_exact_boundaries ... ok
test manifest::tests::test_manifest_roundtrip ... ok
test archiver::tests::test_pack_directory_fast_zstd ... ok
test archiver::tests::test_pack_single_file_store ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 4 tests
test test_exact_chunk_boundary_no_empty_trailing_part ... ok
test test_posix_pipeline_compatibility ... ok
test test_split_nested_directory_fast_zstd ... ok
test test_split_single_large_file_across_chunks ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
```

CLI Help Verification:
```text
Split a file or directory into bounded chunk parts

Usage: fastchunk split [OPTIONS] --output <OUTPUT> --chunk-size <CHUNK_SIZE> <SOURCE>

Arguments:
  <SOURCE>  Path to directory or file to split

Options:
  -o, --output <OUTPUT>          Directory where chunks and manifest.json will be written
  -s, --chunk-size <CHUNK_SIZE>  Chunk size with units (e.g. 14G, 14GB, 4000MB, 500M, 1GiB)
  -m, --mode <MODE>              Compression mode: 'store' (raw streaming) or 'fast' (zstd level 1) [default: fast] [possible values: store, fast]
  -v, --verbose                  Enable verbose diagnostic logging
  -h, --help                     Print help
```

## Atomic Commit History
- `0c89245` - `feat(01-01): initialize fastchunk cargo package and module scaffolding`
- `465d59f` - `feat(01-01): implement human-friendly size parser and CLI arguments`
- `fa5206e` - `feat(01-01): implement streaming ChunkedWriter with on-the-fly SHA-256`
- `fe04051` - `feat(01-01): implement streaming archive packager`
- `0784990` - `feat(01-01): implement manifest generation and CLI split execution`
- `880af03` - `test(01-01): comprehensive integration tests for multi-part slicing and extraction`
