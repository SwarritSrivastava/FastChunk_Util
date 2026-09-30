# Phase 3: Validation, Large Dataset Benchmarking & Ergonomics - Technical Research

## Executive Summary
Phase 3 closes out the project with rigorous large-dataset validation, single-pen-drive shuttle simulation tests, binary bundling ergonomics, and documentation.

---

## 1. `--bundle-executable` Implementation
Using `std::env::current_exe()`, the running `fastchunk` binary can identify its own location on disk and copy itself into the destination chunks directory:
```rust
if bundle_executable {
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_name = exe_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("fastchunk"));
        let target_exe = output_dir.join(exe_name);
        let _ = std::fs::copy(&exe_path, &target_exe);
    }
}
```
This ensures that when a user moves the chunks folder to a flash drive, the `fastchunk` executable is right there next to `restore.sh` and `restore.bat`.

---

## 2. Shuttle Simulation Strategy
In `tests/shuttle_simulation_tests.rs`:
1. Generate test dataset (e.g. 30MB of mock files and subdirectories).
2. Split into 5MB chunks (`data.part001` through `data.part006`).
3. Simulate a single 5MB pen drive:
   - Create a simulated `pen_drive/` directory.
   - For each part `i` from 1 to 6:
     - Copy `manifest.json`, `restore.sh`, and `data.part{i}` to `pen_drive/`.
     - Move from `pen_drive/` to `destination/`.
     - Clear `pen_drive/`.
4. Attempt `restore` at `destination/` and verify bit-for-bit identity across the entire restored tree.

---

## 3. High-Throughput Extraction Benchmarking
In `tests/benchmark_tests.rs`:
- Create a 100MB stream test.
- Split in Store mode and Fast Zstd mode.
- Measure elapsed restoration time using `std::time::Instant`.
- Verify extraction throughput exceeds 500MB/s (typically 1.5–3+ GB/s on modern SSDs), proving extraction in seconds.
