use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PartInfo {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

pub struct ChunkedWriter {
    out_dir: PathBuf,
    base_name: String,
    chunk_size: u64,
    current_part_idx: usize,
    current_chunk_written: u64,
    current_hasher: Sha256,
    current_file: Option<File>,
    completed_parts: Vec<PartInfo>,
}

impl ChunkedWriter {
    pub fn new(out_dir: impl AsRef<Path>, base_name: &str, chunk_size: u64) -> io::Result<Self> {
        let out_dir = out_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&out_dir)?;

        let mut writer = Self {
            out_dir,
            base_name: base_name.to_string(),
            chunk_size,
            current_part_idx: 1,
            current_chunk_written: 0,
            current_hasher: Sha256::new(),
            current_file: None,
            completed_parts: Vec::new(),
        };

        writer.open_next_part()?;
        Ok(writer)
    }

    fn open_next_part(&mut self) -> io::Result<()> {
        let part_filename = format!("{}.part{:03}", self.base_name, self.current_part_idx);
        let part_path = self.out_dir.join(&part_filename);
        let file = File::create(&part_path)?;
        self.current_file = Some(file);
        self.current_chunk_written = 0;
        self.current_hasher = Sha256::new();
        Ok(())
    }

    fn close_current_part(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.current_file.take() {
            file.flush()?;
            let hasher = std::mem::replace(&mut self.current_hasher, Sha256::new());
            let hash_bytes = hasher.finalize();
            let hash_str = hex::encode(hash_bytes);
            let part_filename = format!("{}.part{:03}", self.base_name, self.current_part_idx);
            self.completed_parts.push(PartInfo {
                filename: part_filename,
                size: self.current_chunk_written,
                sha256: hash_str,
            });
            self.current_part_idx += 1;
            self.current_chunk_written = 0;
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<Vec<PartInfo>> {
        if self.current_file.is_some() {
            if self.current_chunk_written > 0 || self.completed_parts.is_empty() {
                self.close_current_part()?;
            } else {
                // If a part file was opened with 0 bytes written and we already have completed parts, remove the empty file
                if let Some(file) = self.current_file.take() {
                    drop(file);
                    let part_filename = format!("{}.part{:03}", self.base_name, self.current_part_idx);
                    let part_path = self.out_dir.join(&part_filename);
                    let _ = std::fs::remove_file(part_path);
                }
            }
        }
        Ok(self.completed_parts)
    }
}

impl Write for ChunkedWriter {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let total_bytes = buf.len();
        while !buf.is_empty() {
            let remaining_in_chunk = self.chunk_size.saturating_sub(self.current_chunk_written);
            if remaining_in_chunk == 0 {
                self.close_current_part()?;
                self.open_next_part()?;
                continue;
            }

            let to_write = std::cmp::min(buf.len() as u64, remaining_in_chunk) as usize;
            let slice = &buf[..to_write];

            if let Some(file) = &mut self.current_file {
                file.write_all(slice)?;
            }
            self.current_hasher.update(slice);
            self.current_chunk_written += to_write as u64;

            buf = &buf[to_write..];
        }
        Ok(total_bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.current_file {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_multipart_splitting_exact_boundaries() {
        let temp = tempdir().unwrap();
        let chunk_size = 400;
        let mut writer = ChunkedWriter::new(temp.path(), "data", chunk_size).unwrap();

        // Write 1000 bytes: 400 + 400 + 200
        let data = vec![0xAB; 1000];
        writer.write_all(&data).unwrap();
        let parts = writer.finish().unwrap();

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].filename, "data.part001");
        assert_eq!(parts[0].size, 400);
        assert_eq!(parts[1].filename, "data.part002");
        assert_eq!(parts[1].size, 400);
        assert_eq!(parts[2].filename, "data.part003");
        assert_eq!(parts[2].size, 200);

        // Verify SHA-256 for each part against disk file
        for part in &parts {
            let file_path = temp.path().join(&part.filename);
            let mut file = File::open(&file_path).unwrap();
            let mut content = Vec::new();
            file.read_to_end(&mut content).unwrap();

            assert_eq!(content.len() as u64, part.size);
            let expected_hash = hex::encode(Sha256::digest(&content));
            assert_eq!(part.sha256, expected_hash);
        }
    }

    #[test]
    fn test_exact_multiple_no_trailing_empty_part() {
        let temp = tempdir().unwrap();
        let chunk_size = 400;
        let mut writer = ChunkedWriter::new(temp.path(), "data", chunk_size).unwrap();

        // Write exactly 800 bytes (2 * 400)
        let data = vec![0xCD; 800];
        writer.write_all(&data).unwrap();
        let parts = writer.finish().unwrap();

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].filename, "data.part001");
        assert_eq!(parts[0].size, 400);
        assert_eq!(parts[1].filename, "data.part002");
        assert_eq!(parts[1].size, 400);

        // Ensure part003 does not exist on disk
        let part3_path = temp.path().join("data.part003");
        assert!(!part3_path.exists());
    }

    #[test]
    fn test_empty_stream_produces_single_empty_part() {
        let temp = tempdir().unwrap();
        let chunk_size = 1000;
        let writer = ChunkedWriter::new(temp.path(), "data", chunk_size).unwrap();
        let parts = writer.finish().unwrap();

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].filename, "data.part001");
        assert_eq!(parts[0].size, 0);

        let empty_hash = hex::encode(Sha256::digest(b""));
        assert_eq!(parts[0].sha256, empty_hash);

        let file_path = temp.path().join("data.part001");
        assert!(file_path.exists());
        assert_eq!(std::fs::metadata(file_path).unwrap().len(), 0);
    }
}
