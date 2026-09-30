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
}
