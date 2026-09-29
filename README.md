# fastchunk

A command-line tool to split large files or directories into fixed-size chunks and restore them on another machine without extra dependencies.

## Features

- Splits directories or single large files into sized parts (e.g. 500M, 14G).
- Generates self-contained restore scripts for Linux, macOS, and Windows.
- Automatically checks for missing parts before extracting.
- Two modes:
  - `fast` (default): Uses zstd compression.
  - `store`: Uncompressed streaming tar.
- Optional `--bundle-executable` flag copies the `fastchunk` binary into the output folder.

## Build

Requires Rust and Cargo.

```bash
cargo build --release
```

The compiled binary will be at `target/release/fastchunk`.

## Usage

### Splitting

Split a folder or file into chunks:

```bash
fastchunk split <SOURCE> -o <OUTPUT_DIR> -s <CHUNK_SIZE>
```

Example:
```bash
fastchunk split /path/to/data -o /path/to/chunks -s 14G -b
```

This generates:
- `data.part001`, `data.part002`, ...
- `manifest.json`: List of parts, sizes, and SHA-256 checksums.
- `restore.sh`: Restore script for Linux and macOS.
- `restore.bat` and `restore.ps1`: Restore scripts for Windows.
- `fastchunk`: Bundled binary (if `-b` is used).

#### Split Options

- `-s, --chunk-size <SIZE>`: Size of each chunk (e.g. `500M`, `14G`, `1000MB`).
- `-o, --output <DIR>`: Output directory for chunk files and scripts.
- `-m, --mode <fast|store>`: Compression mode (`fast` or `store`). Default is `fast`.
- `-b, --bundle-executable`: Copies the `fastchunk` binary into the output directory.
- `-v, --verbose`: Prints progress and extra details.

---

### Restoring

#### Option 1: Using the fastchunk binary

```bash
fastchunk restore <PARTS_DIR> -o <TARGET_DIR>
```

Add `--skip-verify` to bypass SHA-256 hash checks if you want faster extraction.

#### Option 2: Using the generated scripts

On Linux or macOS:
```bash
cd /path/to/chunks
./restore.sh /path/to/destination
```
If no destination is provided, it extracts into the current directory.

On Windows:
```cmd
restore.bat C:\path\to\destination
```
If no destination is provided, it extracts into the current directory.

---

### Running Tests

```bash
cargo test
```
