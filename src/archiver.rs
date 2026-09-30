use std::fs::File;
use std::io;
use std::path::Path;

use crate::chunker::{ChunkedWriter, PartInfo};
use crate::cli::CompressionMode;

#[derive(Debug, Clone)]
pub struct PackagingResult {
    pub total_uncompressed_bytes: u64,
    pub total_entries: usize,
    pub parts: Vec<PartInfo>,
}

fn scan_source(source: &Path) -> io::Result<(u64, usize)> {
    let mut total_bytes = 0u64;
    let mut total_entries = 0usize;

    if source.is_file() {
        let meta = source.metadata()?;
        return Ok((meta.len(), 1));
    }

    if source.is_dir() {
        let mut stack = vec![source.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let ft = entry.file_type()?;
                total_entries += 1;
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() {
                    total_bytes += entry.metadata()?.len();
                }
            }
        }
        return Ok((total_bytes, total_entries));
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Source path '{}' is not a file or directory", source.display()),
    ))
}

pub fn pack_archive(
    source: &Path,
    output_dir: &Path,
    chunk_size: u64,
    mode: CompressionMode,
    verbose: bool,
) -> io::Result<PackagingResult> {
    let (total_uncompressed_bytes, total_entries) = scan_source(source)?;

    if verbose {
        println!(
            "Scanned source: {} entries, {} uncompressed bytes",
            total_entries, total_uncompressed_bytes
        );
    }

    let writer = ChunkedWriter::new(output_dir, "data", chunk_size)?;

    let parts = match mode {
        CompressionMode::Store => {
            let mut builder = tar::Builder::new(writer);
            if source.is_dir() {
                builder.append_dir_all(".", source)?;
            } else if source.is_file() {
                let file_name = source.file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid source file name")
                })?;
                let mut file = File::open(source)?;
                builder.append_file(file_name, &mut file)?;
            }
            builder.finish()?;
            let chunked_writer = builder.into_inner()?;
            chunked_writer.finish()?
        }
        CompressionMode::Fast => {
            let mut encoder = zstd::stream::write::Encoder::new(writer, 1)?;
            let threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1);
            encoder.multithread(threads as u32)?;

            let mut builder = tar::Builder::new(encoder);
            if source.is_dir() {
                builder.append_dir_all(".", source)?;
            } else if source.is_file() {
                let file_name = source.file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid source file name")
                })?;
                let mut file = File::open(source)?;
                builder.append_file(file_name, &mut file)?;
            }
            builder.finish()?;
            let encoder = builder.into_inner()?;
            let chunked_writer = encoder.finish()?;
            chunked_writer.finish()?
        }
    };

    if verbose {
        println!(
            "Packaging complete: {} parts generated in '{}'",
            parts.len(),
            output_dir.display()
        );
    }

    Ok(PackagingResult {
        total_uncompressed_bytes,
        total_entries,
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_pack_single_file_store() {
        let temp_src = tempdir().unwrap();
        let temp_out = tempdir().unwrap();

        let file_path = temp_src.path().join("sample.txt");
        let content = b"Hello, FastChunk streaming engine! Repeat content to reach larger size. "
            .repeat(2000);
        fs::write(&file_path, &content).unwrap();

        let chunk_size = 65536; // 64KB minimum
        let result = pack_archive(
            &file_path,
            temp_out.path(),
            chunk_size,
            CompressionMode::Store,
            false,
        )
        .unwrap();

        assert_eq!(result.total_uncompressed_bytes, content.len() as u64);
        assert_eq!(result.total_entries, 1);
        assert!(!result.parts.is_empty());

        // Reconstruct stream by concatenating parts
        let mut full_archive = Vec::new();
        for part in &result.parts {
            let part_bytes = fs::read(temp_out.path().join(&part.filename)).unwrap();
            full_archive.extend_from_slice(&part_bytes);
        }

        // Unpack with tar
        let mut archive = tar::Archive::new(&full_archive[..]);
        let mut found = false;
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let mut extracted = Vec::new();
            entry.read_to_end(&mut extracted).unwrap();
            assert_eq!(extracted, content);
            found = true;
        }
        assert!(found);
    }

    #[test]
    fn test_pack_directory_fast_zstd() {
        let temp_src = tempdir().unwrap();
        let temp_out = tempdir().unwrap();

        // Create directory structure
        let dir_a = temp_src.path().join("subdir");
        fs::create_dir_all(&dir_a).unwrap();

        let file1 = temp_src.path().join("file1.txt");
        let file2 = dir_a.join("file2.bin");
        let content1 = b"FastChunk directory test content 1";
        let content2 = vec![0x42; 150_000]; // 150KB

        fs::write(&file1, content1).unwrap();
        fs::write(&file2, &content2).unwrap();

        let chunk_size = 65536; // 64KB
        let result = pack_archive(
            temp_src.path(),
            temp_out.path(),
            chunk_size,
            CompressionMode::Fast,
            false,
        )
        .unwrap();

        assert_eq!(
            result.total_uncompressed_bytes,
            (content1.len() + content2.len()) as u64
        );
        assert!(!result.parts.is_empty());

        // Reconstruct by concatenating parts
        let mut compressed = Vec::new();
        for part in &result.parts {
            let part_bytes = fs::read(temp_out.path().join(&part.filename)).unwrap();
            compressed.extend_from_slice(&part_bytes);
        }

        // Decompress with zstd
        let decompressed = zstd::decode_all(&compressed[..]).unwrap();

        // Unpack tar
        let mut archive = tar::Archive::new(&decompressed[..]);
        let temp_restore = tempdir().unwrap();
        archive.unpack(temp_restore.path()).unwrap();

        let restored_file1 = fs::read(temp_restore.path().join("file1.txt")).unwrap();
        let restored_file2 = fs::read(temp_restore.path().join("subdir/file2.bin")).unwrap();

        assert_eq!(restored_file1, content1);
        assert_eq!(restored_file2, content2);
    }
}
