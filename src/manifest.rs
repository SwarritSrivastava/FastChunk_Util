use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::chunker::PartInfo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub created_at: u64,
    pub source_name: String,
    pub source_type: String,
    pub total_uncompressed_bytes: u64,
    pub total_entries: usize,
    pub mode: String,
    pub chunk_size: u64,
    pub parts: Vec<PartInfo>,
}

impl Manifest {
    pub fn new(
        source_name: String,
        source_type: String,
        total_uncompressed_bytes: u64,
        total_entries: usize,
        mode: String,
        chunk_size: u64,
        parts: Vec<PartInfo>,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: 1,
            created_at,
            source_name,
            source_type,
            total_uncompressed_bytes,
            total_entries,
            mode,
            chunk_size,
            parts,
        }
    }

    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let manifest = serde_json::from_reader(file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_manifest_roundtrip() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("manifest.json");

        let parts = vec![
            PartInfo {
                filename: "data.part001".to_string(),
                size: 14000000000,
                sha256: "abcdef1234567890".to_string(),
            },
            PartInfo {
                filename: "data.part002".to_string(),
                size: 2000000000,
                sha256: "1234567890abcdef".to_string(),
            },
        ];

        let manifest = Manifest::new(
            "dataset".to_string(),
            "directory".to_string(),
            16000000000,
            42,
            "fast".to_string(),
            14000000000,
            parts,
        );

        manifest.save_to_file(&manifest_path).unwrap();
        let loaded = Manifest::load_from_file(&manifest_path).unwrap();

        assert_eq!(loaded, manifest);
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.source_name, "dataset");
        assert_eq!(loaded.source_type, "directory");
        assert_eq!(loaded.mode, "fast");
        assert_eq!(loaded.parts.len(), 2);
    }
}
