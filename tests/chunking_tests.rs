use fastchunk::archiver::pack_archive;
use fastchunk::chunker::ChunkedWriter;
use fastchunk::cli::CompressionMode;
use fastchunk::manifest::Manifest;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_split_single_large_file_across_chunks() {
    let temp_src = tempdir().unwrap();
    let temp_out = tempdir().unwrap();

    // 1. Create a 15MB file containing deterministic pseudo-random bytes
    let file_size = 15 * 1024 * 1024; // 15,728,640 bytes
    let mut data = Vec::with_capacity(file_size);
    let mut state: u32 = 0x12345678;
    for _ in 0..file_size {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        data.push((state >> 16) as u8);
    }

    let src_file = temp_src.path().join("large_payload.bin");
    fs::write(&src_file, &data).unwrap();

    // 2. Split with chunk size 4MB (4,194,304 bytes) in Store mode
    let chunk_size = 4 * 1024 * 1024;
    let result = pack_archive(
        &src_file,
        temp_out.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    // 3. Assert 4 parts are generated (data.part001 to data.part004)
    assert_eq!(result.parts.len(), 4);
    assert_eq!(result.parts[0].filename, "data.part001");
    assert_eq!(result.parts[1].filename, "data.part002");
    assert_eq!(result.parts[2].filename, "data.part003");
    assert_eq!(result.parts[3].filename, "data.part004");

    // 4. Assert parts 1-3 are exactly 4,194,304 bytes
    assert_eq!(result.parts[0].size, chunk_size);
    assert_eq!(result.parts[1].size, chunk_size);
    assert_eq!(result.parts[2].size, chunk_size);
    assert!(result.parts[3].size > 0 && result.parts[3].size <= chunk_size);

    // Save manifest as CLI would
    let manifest = Manifest::new(
        "large_payload.bin".to_string(),
        "file".to_string(),
        result.total_uncompressed_bytes,
        result.total_entries,
        "store".to_string(),
        chunk_size,
        result.parts.clone(),
    );
    let manifest_path = temp_out.path().join("manifest.json");
    manifest.save_to_file(&manifest_path).unwrap();

    // 5. Read and verify manifest.json records and hashes against disk files
    let loaded_manifest = Manifest::load_from_file(&manifest_path).unwrap();
    assert_eq!(loaded_manifest.parts.len(), 4);
    for part in &loaded_manifest.parts {
        let part_path = temp_out.path().join(&part.filename);
        assert!(part_path.exists());
        let disk_bytes = fs::read(&part_path).unwrap();
        assert_eq!(disk_bytes.len() as u64, part.size);

        let calculated_hash = hex::encode(Sha256::digest(&disk_bytes));
        assert_eq!(part.sha256, calculated_hash);
    }

    // 6. Concatenate parts into stream, unpack with tar::Archive, verify byte-for-byte identity
    let mut full_stream = Vec::new();
    for part in &loaded_manifest.parts {
        let part_bytes = fs::read(temp_out.path().join(&part.filename)).unwrap();
        full_stream.extend_from_slice(&part_bytes);
    }

    let mut archive = tar::Archive::new(&full_stream[..]);
    let mut restored_count = 0;
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let mut extracted_content = Vec::new();
        entry.read_to_end(&mut extracted_content).unwrap();
        assert_eq!(extracted_content.len(), data.len());
        assert_eq!(extracted_content, data);
        restored_count += 1;
    }
    assert_eq!(restored_count, 1);
}

#[test]
fn test_split_nested_directory_fast_zstd() {
    let temp_src = tempdir().unwrap();
    let temp_out = tempdir().unwrap();

    // Create nested directory structure
    let dir_nested = temp_src.path().join("sub/nested");
    fs::create_dir_all(&dir_nested).unwrap();

    let text_content = b"Hello from FastChunk recursive directory compression test!";
    let binary_size = 400_000; // 400KB pseudo-random binary data to exceed 128KB compressed
    let mut binary_content = Vec::with_capacity(binary_size);
    let mut rand_state: u32 = 0x98765432;
    for _ in 0..binary_size {
        rand_state = rand_state.wrapping_mul(1664525).wrapping_add(1013904223);
        binary_content.push((rand_state >> 16) as u8);
    }
    let deep_text = b"Deeply nested file content inside sub/nested/file3.txt";

    fs::write(temp_src.path().join("file1.txt"), text_content).unwrap();
    fs::write(temp_src.path().join("empty.txt"), b"").unwrap();
    fs::write(temp_src.path().join("sub/file2.bin"), &binary_content).unwrap();
    fs::write(dir_nested.join("file3.txt"), deep_text).unwrap();

    // Split using mode fast with 128KB chunk size
    let chunk_size = 128 * 1024; // 131,072 bytes
    let result = pack_archive(
        temp_src.path(),
        temp_out.path(),
        chunk_size,
        CompressionMode::Fast,
        false,
    )
    .unwrap();

    assert!(result.parts.len() >= 2);
    let manifest = Manifest::new(
        "src".to_string(),
        "directory".to_string(),
        result.total_uncompressed_bytes,
        result.total_entries,
        "fast".to_string(),
        chunk_size,
        result.parts.clone(),
    );
    let manifest_path = temp_out.path().join("manifest.json");
    manifest.save_to_file(&manifest_path).unwrap();

    let loaded = Manifest::load_from_file(&manifest_path).unwrap();
    assert_eq!(loaded.mode, "fast");

    // Concatenate parts sequentially into zstd::Decoder and unpack with tar
    let mut compressed_stream = Vec::new();
    for part in &loaded.parts {
        let part_bytes = fs::read(temp_out.path().join(&part.filename)).unwrap();
        compressed_stream.extend_from_slice(&part_bytes);
    }

    let decoder = zstd::stream::read::Decoder::new(&compressed_stream[..]).unwrap();
    let mut archive = tar::Archive::new(decoder);
    let temp_restore = tempdir().unwrap();
    archive.unpack(temp_restore.path()).unwrap();

    // Verify all restored files match original contents
    assert_eq!(
        fs::read(temp_restore.path().join("file1.txt")).unwrap(),
        text_content
    );
    assert_eq!(
        fs::read(temp_restore.path().join("empty.txt")).unwrap(),
        b""
    );
    assert_eq!(
        fs::read(temp_restore.path().join("sub/file2.bin")).unwrap(),
        binary_content
    );
    assert_eq!(
        fs::read(temp_restore.path().join("sub/nested/file3.txt")).unwrap(),
        deep_text
    );
}

#[test]
fn test_exact_chunk_boundary_no_empty_trailing_part() {
    let temp = tempdir().unwrap();
    let chunk_size = 65536; // 64KB
    let mut writer = ChunkedWriter::new(temp.path(), "data", chunk_size).unwrap();

    // Write exact multiple: 2 * 65536 bytes
    let payload = vec![0x55; (chunk_size * 2) as usize];
    writer.write_all(&payload).unwrap();
    let parts = writer.finish().unwrap();

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].size, chunk_size);
    assert_eq!(parts[1].size, chunk_size);

    // Assert that data.part003 does NOT exist on disk
    let part3 = temp.path().join("data.part003");
    assert!(!part3.exists(), "part003 should not exist when stream lands exactly on chunk boundary");
}

#[test]
#[cfg(unix)]
fn test_posix_pipeline_compatibility() {
    let temp_src = tempdir().unwrap();
    let temp_store_out = tempdir().unwrap();
    let temp_fast_out = tempdir().unwrap();

    let sample_text = b"Posix standard streaming pipeline verification payload.";
    fs::write(temp_src.path().join("posix.txt"), sample_text).unwrap();

    let chunk_size = 65536;

    // 1. Store mode pipeline: cat data.part* | tar -tf -
    let _store_result = pack_archive(
        temp_src.path(),
        temp_store_out.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    let store_cmd = format!(
        "cat {}/data.part* | tar -tf -",
        temp_store_out.path().display()
    );
    let output_store = Command::new("sh")
        .arg("-c")
        .arg(&store_cmd)
        .output()
        .expect("Failed to execute store pipeline");

    assert!(
        output_store.status.success(),
        "POSIX tar pipeline failed on store mode: {}",
        String::from_utf8_lossy(&output_store.stderr)
    );
    let stdout_store = String::from_utf8_lossy(&output_store.stdout);
    assert!(stdout_store.contains("posix.txt"));

    // 2. Fast mode pipeline: cat data.part* | zstd -d | tar -tf -
    let _fast_result = pack_archive(
        temp_src.path(),
        temp_fast_out.path(),
        chunk_size,
        CompressionMode::Fast,
        false,
    )
    .unwrap();

    let fast_cmd = format!(
        "cat {}/data.part* | zstd -d | tar -tf -",
        temp_fast_out.path().display()
    );
    let output_fast = Command::new("sh")
        .arg("-c")
        .arg(&fast_cmd)
        .output()
        .expect("Failed to execute fast pipeline");

    assert!(
        output_fast.status.success(),
        "POSIX zstd pipeline failed on fast mode: {}",
        String::from_utf8_lossy(&output_fast.stderr)
    );
    let stdout_fast = String::from_utf8_lossy(&output_fast.stdout);
    assert!(stdout_fast.contains("posix.txt"));
}
