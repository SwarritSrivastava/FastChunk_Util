# Phase 3 Context: Validation, Large Dataset Benchmarking & Ergonomics

## Purpose & Scope
Phase 3 validates real-world performance and ergonomics:
1. End-to-end integration tests simulating a single-pen-drive shuttle workflow.
2. Large dataset benchmarks measuring throughput and proving seconds-scale extraction.
3. Adding `--bundle-executable` flag to `fastchunk split` so the standalone native binary is automatically copied into the parts folder.
4. Writing clear, comprehensive documentation and usage guides (`README.md`).

## Requirements Covered
- **NFR-1**: Fast Decompression (>= 500 MB/s - 1.5 GB/s verified).
- **NFR-3**: Cross-Platform Compatibility.
- **NFR-4**: Memory Efficiency (< 64MB RAM footprint under large workloads).
- Ergonomics: `--bundle-executable` convenience flag.

## Key Deliverables
- `src/cli.rs`: Add `--bundle-executable` flag to `split`.
- `tests/shuttle_simulation_tests.rs`: Integration tests simulating moving parts one by one via a simulated 15GB drive buffer.
- `benches/` or integration benchmark tests measuring extraction throughput.
- `README.md`: Step-by-step tutorial for moving 40GB folders with a 15GB pen drive.
