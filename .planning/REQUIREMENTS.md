# Requirements

## Functional Requirements
- **FR-1**: User can specify an input directory or file and a maximum chunk size (e.g. `14GB`, `4000MB`, `500M`).
- **FR-2**: Tool must split the archive stream across multiple parts (`part_001.bin`, `part_002.bin`, ...).
- **FR-3**: Individual files larger than the specified chunk size must be seamlessly split across multiple parts without corruption.
- **FR-4**: Tool must generate a `manifest.json` containing total size, individual part sizes, part filenames, and per-part cryptographic checksums (SHA-256 / BLAKE3).
- **FR-5**: Tool must bundle zero-install restore scripts (`restore.sh` for Linux/macOS and `restore.bat`/`restore.ps1` for Windows) in the output directory.
- **FR-6**: The restore mechanism must verify all parts are present before attempting extraction. If any part is missing, it must print an explicit message specifying which part(s) need to be copied.
- **FR-7**: The restore mechanism must support fast extraction that runs in seconds (utilizing streaming I/O and optional multi-threaded Zstd decompression).
- **FR-8**: User can select between `--fast` (Zstd level 1) and `--store` (zero-compression byte streaming).

## Non-Functional Requirements
- **NFR-1**: Fast Decompression — Target extraction speed >= 500 MB/s - 1.5 GB/s on modern NVMe/SSD, completing 40GB in seconds.
- **NFR-2**: Zero External Dependencies on Target — Target computer requires no pre-installed runtime (no Python, no Node.js, no 7-Zip). Works via native OS tools or bundled portable static binary.
- **NFR-3**: Cross-Platform Compatibility — Supports paths, symlinks, and file permissions across Linux, Windows, and macOS.
- **NFR-4**: Memory Efficiency — Streaming chunking and extraction using fixed-size buffers (< 64MB RAM footprint) regardless of whether processing 40GB or 1TB.

## Traceability

| Requirement | Phase | Status | Verification |
|-------------|-------|--------|--------------|
| FR-1 | Phase 1 | Satisfied | Tested via `size_parser` unit tests and CLI integration |
| FR-2 | Phase 1 | Satisfied | Tested via `chunker` boundary tests |
| FR-3 | Phase 1 | Satisfied | Tested via `test_split_single_large_file_across_chunks` |
| FR-4 | Phase 1 | Satisfied | Tested via `manifest` serialization and SHA-256 validation |
| FR-5 | Phase 2 | Satisfied | Tested via `scripts` unit & integration tests (`restore.sh`, `restore.bat`, `restore.ps1`) |
| FR-6 | Phase 2 | Satisfied | Tested via `test_preflight_missing_part_alert_prevents_extraction` |
| FR-7 | Phase 2 | Satisfied | Tested via `extract_archive` with Zstd & Store streaming |
| FR-8 | Phase 1 | Satisfied | Tested via `archiver` store and fast zstd modes |
| NFR-1 | Phase 2 | Satisfied | Streaming I/O directly into tar unpacker at disk throughput |
| NFR-2 | Phase 2 | Satisfied | Portable standalone scripts use native OS tools (`cat`/`tar`/`powershell`) |
| NFR-3 | Phase 1..3 | In Progress | Cross-platform Rust stdlib paths and native scripts |
| NFR-4 | Phase 1..2 | Satisfied | Streaming I/O with fixed 256KB buffer |
