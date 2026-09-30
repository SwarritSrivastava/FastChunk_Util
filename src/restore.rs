use sha2::{Digest, Sha256};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::manifest::Manifest;

#[derive(Debug)]
pub enum RestoreError {
    Io(io::Error),
    MissingParts(Vec<String>),
    CorruptParts(Vec<String>),
    ChecksumMismatch(String),
    ManifestError(String),
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RestoreError::Io(err) => write!(f, "IO error: {}", err),
            RestoreError::MissingParts(parts) => {
                write!(f, "Missing part(s):\n  - {}", parts.join("\n  - "))
            }
            RestoreError::CorruptParts(parts) => {
                write!(f, "Corrupted part(s):\n  - {}", parts.join("\n  - "))
            }
            RestoreError::ChecksumMismatch(msg) => write!(f, "Checksum mismatch: {}", msg),
            RestoreError::ManifestError(msg) => write!(f, "Manifest error: {}", msg),
        }
    }
}

impl std::error::Error for RestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RestoreError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for RestoreError {
    fn from(err: io::Error) -> Self {
        RestoreError::Io(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingPartInfo {
    pub part_index: usize,
    pub filename: String,
    pub expected_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorruptPartInfo {
    pub filename: String,
    pub expected_size: u64,
    pub actual_size: u64,
}

pub fn verify_parts_exist(parts_dir: &Path, manifest: &Manifest) -> Result<(), RestoreError> {
    let mut missing_parts = Vec::new();
    let mut corrupt_parts = Vec::new();

    for (idx, part) in manifest.parts.iter().enumerate() {
        let part_index = idx + 1;
        let part_path = parts_dir.join(&part.filename);

        if !part_path.exists() {
            missing_parts.push(MissingPartInfo {
                part_index,
                filename: part.filename.clone(),
                expected_size: part.size,
            });
        } else {
            let actual_size = match part_path.metadata() {
                Ok(meta) => meta.len(),
                Err(err) => return Err(RestoreError::Io(err)),
            };

            if actual_size != part.size {
                corrupt_parts.push(CorruptPartInfo {
                    filename: part.filename.clone(),
                    expected_size: part.size,
                    actual_size,
                });
            }
        }
    }

    if !missing_parts.is_empty() {
        let formatted_missing: Vec<String> = missing_parts
            .into_iter()
            .map(|m| {
                format!(
                    "Missing: {} (part {}, expected size: {} bytes)",
                    m.filename, m.part_index, m.expected_size
                )
            })
            .collect();
        return Err(RestoreError::MissingParts(formatted_missing));
    }

    if !corrupt_parts.is_empty() {
        let formatted_corrupt: Vec<String> = corrupt_parts
            .into_iter()
            .map(|c| {
                format!(
                    "Corrupt size: {} (expected {} bytes, found {} bytes)",
                    c.filename, c.expected_size, c.actual_size
                )
            })
            .collect();
        return Err(RestoreError::CorruptParts(formatted_corrupt));
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct RestoreOptions {
    pub skip_verify: bool,
    pub verbose: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            skip_verify: false,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub total_entries_restored: usize,
    pub total_bytes_uncompressed: u64,
    pub source_name: String,
    pub duration: Duration,
}

pub struct ChunkedReader {
    part_paths: Vec<PathBuf>,
    current_idx: usize,
    current_file: Option<std::fs::File>,
}

impl ChunkedReader {
    pub fn new(part_paths: Vec<PathBuf>) -> io::Result<Self> {
        let mut reader = Self {
            part_paths,
            current_idx: 0,
            current_file: None,
        };
        reader.open_current()?;
        Ok(reader)
    }

    fn open_current(&mut self) -> io::Result<()> {
        if self.current_idx < self.part_paths.len() {
            let path = &self.part_paths[self.current_idx];
            let file = std::fs::File::open(path)?;
            self.current_file = Some(file);
        } else {
            self.current_file = None;
        }
        Ok(())
    }
}

impl io::Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            let file = match &mut self.current_file {
                Some(f) => f,
                None => return Ok(0),
            };

            let bytes_read = file.read(buf)?;
            if bytes_read > 0 {
                return Ok(bytes_read);
            }

            self.current_idx += 1;
            self.open_current()?;
        }
    }
}

fn verify_checksums(parts_dir: &Path, manifest: &Manifest) -> Result<(), RestoreError> {
    for part in &manifest.parts {
        let part_path = parts_dir.join(&part.filename);
        let mut file = std::fs::File::open(&part_path)?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)?;
        let actual_hash = hex::encode(hasher.finalize());
        if actual_hash != part.sha256 {
            return Err(RestoreError::ChecksumMismatch(format!(
                "Part '{}' checksum mismatch: expected {}, got {}",
                part.filename, part.sha256, actual_hash
            )));
        }
    }
    Ok(())
}

pub fn extract_archive(
    parts_dir: &Path,
    target_dir: &Path,
    options: &RestoreOptions,
) -> Result<RestoreResult, RestoreError> {
    let manifest_path = parts_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(RestoreError::ManifestError(format!(
            "manifest.json not found in '{}'",
            parts_dir.display()
        )));
    }

    let manifest = Manifest::load_from_file(&manifest_path)
        .map_err(|e| RestoreError::ManifestError(e.to_string()))?;

    let start_time = std::time::Instant::now();

    // Step 1: Pre-flight check
    verify_parts_exist(parts_dir, &manifest)?;

    // Step 2: Checksum verification
    if !options.skip_verify {
        if options.verbose {
            println!("Verifying part checksums...");
        }
        verify_checksums(parts_dir, &manifest)?;
    }

    // Step 3: Collect ordered part paths
    let part_paths: Vec<PathBuf> = manifest
        .parts
        .iter()
        .map(|p| parts_dir.join(&p.filename))
        .collect();

    // Step 4: Chunked reader
    let chunked_reader = ChunkedReader::new(part_paths)?;

    // Step 5: Unpack TAR archive
    std::fs::create_dir_all(target_dir)?;

    if options.verbose {
        println!(
            "Unpacking archive to '{}' (mode: {})...",
            target_dir.display(),
            manifest.mode
        );
    }

    match manifest.mode.as_str() {
        "store" => {
            let mut archive = tar::Archive::new(chunked_reader);
            archive.unpack(target_dir)?;
        }
        "fast" => {
            let zstd_decoder = zstd::stream::read::Decoder::new(chunked_reader)?;
            let mut archive = tar::Archive::new(zstd_decoder);
            archive.unpack(target_dir)?;
        }
        other => {
            return Err(RestoreError::ManifestError(format!(
                "Unsupported compression mode '{}' in manifest",
                other
            )));
        }
    }

    Ok(RestoreResult {
        total_entries_restored: manifest.total_entries,
        total_bytes_uncompressed: manifest.total_uncompressed_bytes,
        source_name: manifest.source_name,
        duration: start_time.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::PartInfo;
    use std::fs;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_preflight_all_present() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        let p2 = temp.path().join("data.part002");
        fs::write(&p1, vec![0u8; 100]).unwrap();
        fs::write(&p2, vec![0u8; 200]).unwrap();

        let manifest = Manifest::new(
            "test".to_string(),
            "file".to_string(),
            300,
            1,
            "store".to_string(),
            100,
            vec![
                PartInfo {
                    filename: "data.part001".to_string(),
                    size: 100,
                    sha256: "hash1".to_string(),
                },
                PartInfo {
                    filename: "data.part002".to_string(),
                    size: 200,
                    sha256: "hash2".to_string(),
                },
            ],
        );

        assert!(verify_parts_exist(temp.path(), &manifest).is_ok());
    }

    #[test]
    fn test_preflight_missing_part() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        fs::write(&p1, vec![0u8; 100]).unwrap();
        // data.part002 is deliberately not created

        let manifest = Manifest::new(
            "test".to_string(),
            "file".to_string(),
            300,
            1,
            "store".to_string(),
            100,
            vec![
                PartInfo {
                    filename: "data.part001".to_string(),
                    size: 100,
                    sha256: "hash1".to_string(),
                },
                PartInfo {
                    filename: "data.part002".to_string(),
                    size: 200,
                    sha256: "hash2".to_string(),
                },
            ],
        );

        let res = verify_parts_exist(temp.path(), &manifest);
        match res {
            Err(RestoreError::MissingParts(parts)) => {
                assert_eq!(parts.len(), 1);
                assert!(parts[0].contains("data.part002"));
                assert!(parts[0].contains("expected size: 200 bytes"));
            }
            other => panic!("Expected MissingParts error, got {:?}", other),
        }
    }

    #[test]
    fn test_preflight_size_mismatch() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        fs::write(&p1, vec![0u8; 50]).unwrap(); // Expected 100, actual 50

        let manifest = Manifest::new(
            "test".to_string(),
            "file".to_string(),
            100,
            1,
            "store".to_string(),
            100,
            vec![PartInfo {
                filename: "data.part001".to_string(),
                size: 100,
                sha256: "hash1".to_string(),
            }],
        );

        let res = verify_parts_exist(temp.path(), &manifest);
        match res {
            Err(RestoreError::CorruptParts(parts)) => {
                assert_eq!(parts.len(), 1);
                assert!(parts[0].contains("data.part001"));
                assert!(parts[0].contains("expected 100 bytes, found 50 bytes"));
            }
            other => panic!("Expected CorruptParts error, got {:?}", other),
        }
    }
    #[test]
    fn test_chunked_reader_sequential_read() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        let p2 = temp.path().join("data.part002");
        let p3 = temp.path().join("data.part003");

        let b1: Vec<u8> = (0..100).collect();
        let b2: Vec<u8> = (100..200).collect();
        let b3: Vec<u8> = (200..255).collect();

        fs::write(&p1, &b1).unwrap();
        fs::write(&p2, &b2).unwrap();
        fs::write(&p3, &b3).unwrap();

        let mut reader = ChunkedReader::new(vec![p1, p2, p3]).unwrap();
        let mut out = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }

        let mut expected = Vec::new();
        expected.extend_from_slice(&b1);
        expected.extend_from_slice(&b2);
        expected.extend_from_slice(&b3);

        assert_eq!(out, expected);
    }

    #[test]
    fn test_chunked_reader_small_buffer_reads() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        let p2 = temp.path().join("data.part002");

        let b1 = b"ABCDEFGHIJ"; // 10 bytes
        let b2 = b"KLMNOPQRST"; // 10 bytes

        fs::write(&p1, b1).unwrap();
        fs::write(&p2, b2).unwrap();

        let mut reader = ChunkedReader::new(vec![p1, p2]).unwrap();
        let mut out = Vec::new();
        let mut buf = [0u8; 7]; // 7 bytes buffer, forces cross-boundary reads

        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }

        assert_eq!(&out, b"ABCDEFGHIJKLMNOPQRST");
    }

    #[test]
    fn test_chunked_reader_empty_parts() {
        let temp = tempdir().unwrap();
        let p1 = temp.path().join("data.part001");
        let p2 = temp.path().join("data.part002");
        let p3 = temp.path().join("data.part003");

        fs::write(&p1, b"").unwrap();
        fs::write(&p2, b"hello").unwrap();
        fs::write(&p3, b"").unwrap();

        let mut reader = ChunkedReader::new(vec![p1, p2, p3]).unwrap();
        let mut out = Vec::new();
        let mut buf = [0u8; 16];
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }

        assert_eq!(&out, b"hello");
    }

    #[test]
    fn test_extract_archive_store() {
        use crate::archiver::pack_archive;
        use crate::cli::CompressionMode;

        let temp_src = tempdir().unwrap();
        let temp_out = tempdir().unwrap();
        let temp_dest = tempdir().unwrap();

        let file1 = temp_src.path().join("file1.txt");
        let sub = temp_src.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        let file2 = sub.join("file2.bin");

        let c1 = b"Hello FastChunk store extraction test!";
        let c2 = vec![0x99; 2000];
        fs::write(&file1, c1).unwrap();
        fs::write(&file2, &c2).unwrap();

        let pack_res = pack_archive(
            temp_src.path(),
            temp_out.path(),
            500,
            CompressionMode::Store,
            false,
        )
        .unwrap();

        let manifest = Manifest::new(
            "test".to_string(),
            "directory".to_string(),
            pack_res.total_uncompressed_bytes,
            pack_res.total_entries,
            "store".to_string(),
            500,
            pack_res.parts,
        );
        manifest
            .save_to_file(&temp_out.path().join("manifest.json"))
            .unwrap();

        let restore_res = extract_archive(
            temp_out.path(),
            temp_dest.path(),
            &RestoreOptions::default(),
        )
        .unwrap();

        assert_eq!(restore_res.total_entries_restored, pack_res.total_entries);
        assert_eq!(
            restore_res.total_bytes_uncompressed,
            pack_res.total_uncompressed_bytes
        );

        let r1 = fs::read(temp_dest.path().join("file1.txt")).unwrap();
        let r2 = fs::read(temp_dest.path().join("sub/file2.bin")).unwrap();
        assert_eq!(r1, c1);
        assert_eq!(r2, c2);
    }

    #[test]
    fn test_extract_archive_fast() {
        use crate::archiver::pack_archive;
        use crate::cli::CompressionMode;

        let temp_src = tempdir().unwrap();
        let temp_out = tempdir().unwrap();
        let temp_dest = tempdir().unwrap();

        let file1 = temp_src.path().join("file1.txt");
        let sub = temp_src.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        let file2 = sub.join("file2.bin");

        let c1 = b"Hello FastChunk fast zstd extraction test!";
        let c2 = vec![0x77; 5000];
        fs::write(&file1, c1).unwrap();
        fs::write(&file2, &c2).unwrap();

        let pack_res = pack_archive(
            temp_src.path(),
            temp_out.path(),
            1000,
            CompressionMode::Fast,
            false,
        )
        .unwrap();

        let manifest = Manifest::new(
            "test".to_string(),
            "directory".to_string(),
            pack_res.total_uncompressed_bytes,
            pack_res.total_entries,
            "fast".to_string(),
            1000,
            pack_res.parts,
        );
        manifest
            .save_to_file(&temp_out.path().join("manifest.json"))
            .unwrap();

        let restore_res = extract_archive(
            temp_out.path(),
            temp_dest.path(),
            &RestoreOptions::default(),
        )
        .unwrap();

        assert_eq!(restore_res.total_entries_restored, pack_res.total_entries);
        assert_eq!(
            restore_res.total_bytes_uncompressed,
            pack_res.total_uncompressed_bytes
        );

        let r1 = fs::read(temp_dest.path().join("file1.txt")).unwrap();
        let r2 = fs::read(temp_dest.path().join("sub/file2.bin")).unwrap();
        assert_eq!(r1, c1);
        assert_eq!(r2, c2);
    }

    #[test]
    fn test_extract_archive_checksum_failure() {
        use crate::archiver::pack_archive;
        use crate::cli::CompressionMode;

        let temp_src = tempdir().unwrap();
        let temp_out = tempdir().unwrap();

        let file1 = temp_src.path().join("payload.txt");
        let content = vec![b'A'; 5000];
        fs::write(&file1, &content).unwrap();

        let pack_res = pack_archive(
            temp_src.path(),
            temp_out.path(),
            3000,
            CompressionMode::Store,
            false,
        )
        .unwrap();

        let manifest = Manifest::new(
            "test".to_string(),
            "directory".to_string(),
            pack_res.total_uncompressed_bytes,
            pack_res.total_entries,
            "store".to_string(),
            3000,
            pack_res.parts.clone(),
        );
        manifest
            .save_to_file(&temp_out.path().join("manifest.json"))
            .unwrap();

        // Corrupt byte 1500 of data.part001 (offset > 1024 bytes so it is inside file content, not tar header)
        let part1_path = temp_out.path().join(&pack_res.parts[0].filename);
        let mut part1_bytes = fs::read(&part1_path).unwrap();
        assert!(part1_bytes.len() > 1500);
        part1_bytes[1500] ^= 0xFF;
        fs::write(&part1_path, &part1_bytes).unwrap();

        // Test with skip_verify: false -> must fail with ChecksumMismatch
        let temp_dest1 = tempdir().unwrap();
        let res_verify = extract_archive(
            temp_out.path(),
            temp_dest1.path(),
            &RestoreOptions {
                skip_verify: false,
                verbose: false,
            },
        );
        match res_verify {
            Err(RestoreError::ChecksumMismatch(msg)) => {
                assert!(msg.contains(&pack_res.parts[0].filename));
            }
            other => panic!("Expected ChecksumMismatch, got {:?}", other),
        }

        // Test with skip_verify: true -> bypasses checksum check
        let temp_dest2 = tempdir().unwrap();
        let res_skip = extract_archive(
            temp_out.path(),
            temp_dest2.path(),
            &RestoreOptions {
                skip_verify: true,
                verbose: false,
            },
        );
        assert!(res_skip.is_ok());
    }
}
