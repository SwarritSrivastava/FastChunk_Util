# Phase 2 Context: Zero-Dependency Setup & Fast Restorer

## Purpose & Scope
Phase 2 implements the destination reconstruction pipeline. When users arrive at the target machine with their split parts (`data.part001`, `data.part002`, ...), they need to reconstruct the original folder/file in seconds without having to install third-party runtimes (no Python, no Node, no 7-Zip).

## User Decisions & Requirements
- **FR-5**: Tool must bundle zero-install restore scripts (`restore.sh` for Linux/macOS and `restore.bat`/`restore.ps1` for Windows) in the output directory alongside `manifest.json`.
- **FR-6**: Pre-flight missing-part validation: Before attempting extraction, the restorer must verify all parts are present. If any part (e.g., part 3 of 4) is missing, it must print an explicit warning identifying the missing part(s) and fail early before touching disk.
- **FR-7**: Fast extraction in seconds using streaming multi-threaded Zstd or raw Store stream reconstitution.
- **NFR-1**: Speed target >= 500MB/s - 1.5GB/s on modern drives.
- **NFR-2**: Zero external dependencies on target machine.

## Architecture & Components
1. **Auto-Generated Setup Scripts**:
   - `restore.sh`:
     - Reads `manifest.json` or scans `data.part*`.
     - Validates all parts exist.
     - Performs streaming reassembly using POSIX tools:
       - For `store` mode: `cat data.part* | tar -xf - -C <DEST>`
       - For `fast` (zstd) mode: checks if `zstd` is available; if not, informs user or falls back to bundled binary.
   - `restore.bat` / `restore.ps1`:
     - Native Windows script using built-in Windows PowerShell (`tar.exe` is built into Windows 10/11 since build 17063).
     - Checks all parts exist and combines stream.
2. **Native Rust `fastchunk restore` Subcommand**:
   - `fastchunk restore <PARTS_DIR> -o <TARGET_DIR> [--skip-verify]`
   - Pre-flight checks:
     - Parses `manifest.json`.
     - Checks every part in `manifest.parts` exists and matches exact byte size.
     - If any part is missing:
       `"Error: Missing part 3 (data.part003). Please copy data.part003 to this folder before restoring."`
     - Optional SHA-256 verification (on by default, can be bypassed with `--skip-verify` for immediate instant unpack).
   - Reconstructs stream:
     - Sequential reader (`ChunkedReader`) chaining part files into a single `std::io::Read`.
     - Decodes Zstd (if `fast` mode) or raw bytes (if `store` mode).
     - Feeds `tar::Archive::unpack(target_dir)`.
3. **Auto-Bundling in `fastchunk split`**:
   - When `fastchunk split` finishes writing parts and `manifest.json`, it automatically generates `restore.sh` and `restore.bat` directly in the output directory.
