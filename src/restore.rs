use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
                write!(f, "Missing part(s):\n{}", parts.join("\n"))
            }
            RestoreError::CorruptParts(parts) => {
                write!(f, "Corrupted part(s):\n{}", parts.join("\n"))
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

    #[test]
    fn test_stub() {
        assert!(true);
    }
}
