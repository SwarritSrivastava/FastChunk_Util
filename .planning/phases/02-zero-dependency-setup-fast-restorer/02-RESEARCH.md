# Phase 2: Zero-Dependency Setup & Fast Restorer - Technical Research

## Executive Summary
Phase 2 builds the restoration pipeline that enables extraction at the destination machine in seconds with zero external dependencies.

The pipeline comprises two synergistic mechanisms:
1. **Self-Contained Setup Scripts (`restore.sh`, `restore.bat`, `restore.ps1`)**:
   Automatically placed in the chunk output directory by `fastchunk split`. Works immediately on clean OS installations using native built-in OS tools (POSIX `cat`/`tar` on Linux/macOS; `tar.exe` / PowerShell on Windows).
2. **Native Rust `fastchunk restore` Subcommand**:
   A high-performance streaming reconstructor compiled directly into the `fastchunk` binary with progress bars, pre-flight part verification, and optional fast SHA-256 integrity validation.

---

## 1. Pre-Flight Missing-Part Detection
When shuttling files on physical media (like a 15GB USB pen drive), the most common failure mode is attempting to extract before all parts are copied to the target directory.

### Detection Algorithm:
1. Load `manifest.json`.
2. Inspect `manifest.parts` (which lists `part_001`, `part_002`, ... up to `part_N`).
3. For each part:
   - Check if `<PARTS_DIR>/<filename>` exists.
   - If not found, add to `missing_parts` vector.
   - If found, check if `file.metadata().len() == part.size`. If mismatch, add to `corrupt_parts` vector.
4. If `missing_parts` is non-empty:
   - Immediately abort before creating directories or touching the target location.
   - Print clear error:
     ```text
     Error: 1 part(s) missing from target directory!
       - Missing: data.part003 (Expected size: 14.00 GB)
     Please copy the missing part(s) into this directory and run restore again.
     ```
5. If all parts exist, proceed to stream extraction (optionally running SHA-256 verification in parallel or sequentially).

---

## 2. Streaming Reconstruction (`ChunkedReader`)
Rather than concatenating 40GB of parts into a temporary file on disk (which would double disk space requirements and waste minutes of I/O), we implement a streaming reader:

```rust
pub struct ChunkedReader {
    part_paths: Vec<PathBuf>,
    current_idx: usize,
    current_file: Option<File>,
}
```
When `std::io::Read::read()` is called:
- If `current_file` has bytes, read into buffer.
- If EOF is reached on `current_file`, close it and open `part_paths[current_idx + 1]`.
- Return when all parts are read.

This gives a seamless continuous `std::io::Read` stream across all chunk files.

### Decompression & Unpack:
- If `mode == CompressionMode::Fast`:
  ```rust
  let zstd_reader = zstd::Decoder::new(chunked_reader)?;
  let mut archive = tar::Archive::new(zstd_reader);
  archive.unpack(target_dir)?;
  ```
- If `mode == CompressionMode::Store`:
  ```rust
  let mut archive = tar::Archive::new(chunked_reader);
  archive.unpack(target_dir)?;
  ```

Because `tar` unpacks sequentially and `zstd` decodes at 1.5 - 2.5 GB/s, reading from modern SSDs unpacks 40GB in **seconds**.

---

## 3. Native Script Generation
When `fastchunk split` completes, it generates:

### `restore.sh` (Linux / macOS):
```bash
#!/usr/bin/env sh
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
MANIFEST="$DIR/manifest.json"
if [ ! -f "$MANIFEST" ]; then
  echo "Error: manifest.json not found in $DIR"
  exit 1
fi
DEST="${1:-.}"
echo "Restoring to: $DEST"
# Check if fastchunk binary is present in same dir
if [ -x "$DIR/fastchunk" ]; then
  "$DIR/fastchunk" restore "$DIR" -o "$DEST"
  exit $?
fi
# Fallback to POSIX tools
cat "$DIR"/data.part* | zstd -d 2>/dev/null | tar -xf - -C "$DEST" 2>/dev/null || cat "$DIR"/data.part* | tar -xf - -C "$DEST"
echo "Restore complete."
```

### `restore.bat` (Windows):
```bat
@echo off
setlocal
set "DIR=%~dp0"
if not "%~1"=="" (set "DEST=%~1") else (set "DEST=.")
if exist "%DIR%fastchunk.exe" (
  "%DIR%fastchunk.exe" restore "%DIR%" -o "%DEST%"
  exit /b %ERRORLEVEL%
)
echo Pre-flight: checking parts...
powershell -NoProfile -ExecutionPolicy Bypass -File "%DIR%restore.ps1" "%DEST%"
```

### `restore.ps1` (Windows PowerShell):
Validates all parts in `manifest.json`, warns if any part is missing, and combines parts directly into `tar.exe -xf - -C <DEST>`.
