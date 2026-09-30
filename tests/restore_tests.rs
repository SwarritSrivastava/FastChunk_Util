use fastchunk::archiver::pack_archive;
use fastchunk::cli::CompressionMode;
use fastchunk::manifest::Manifest;
use fastchunk::restore::*;
use fastchunk::scripts::write_restore_scripts;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn assert_dirs_equal(src: &Path, dst: &Path) {
    let mut src_files = Vec::new();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(src).unwrap().to_path_buf();
                src_files.push(rel);
            }
        }
    }

    for rel in &src_files {
        let src_file = src.join(rel);
        let dst_file = dst.join(rel);
        assert!(
            dst_file.exists(),
            "Missing restored file: {}",
            rel.display()
        );
        let src_bytes = fs::read(&src_file).unwrap();
        let dst_bytes = fs::read(&dst_file).unwrap();
        assert_eq!(
            src_bytes,
            dst_bytes,
            "Content mismatch in: {}",
            rel.display()
        );
    }
}

#[test]
fn test_preflight_missing_part_alert_prevents_extraction() {
    let temp_src = tempdir().unwrap();
    let temp_parts = tempdir().unwrap();
    let temp_dest = tempdir().unwrap();

    // Create test files
    let file = temp_src.path().join("data.bin");
    fs::write(&file, vec![0xAB; 200_000]).unwrap(); // 200KB

    // Split into ~3 parts with 70KB chunk size
    let chunk_size = 70_000;
    let pack_res = pack_archive(
        temp_src.path(),
        temp_parts.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    assert_eq!(pack_res.parts.len(), 3);
    assert_eq!(pack_res.parts[1].filename, "data.part002");

    let manifest = Manifest::new(
        "test_dataset".to_string(),
        "directory".to_string(),
        pack_res.total_uncompressed_bytes,
        pack_res.total_entries,
        "store".to_string(),
        chunk_size,
        pack_res.parts.clone(),
    );
    manifest
        .save_to_file(&temp_parts.path().join("manifest.json"))
        .unwrap();

    // Delete part002 to simulate a missing part
    let part2_path = temp_parts.path().join("data.part002");
    fs::remove_file(&part2_path).unwrap();

    // Run extract_archive
    let res = extract_archive(
        temp_parts.path(),
        temp_dest.path(),
        &RestoreOptions::default(),
    );

    // Verify error identifies missing part
    match res {
        Err(RestoreError::MissingParts(missing)) => {
            assert!(
                missing.iter().any(|m| m.contains("data.part002")),
                "Expected missing parts to list data.part002, got {:?}",
                missing
            );
        }
        other => panic!("Expected MissingParts error, got {:?}", other),
    }

    // Verify destination directory was not populated with extracted files
    let dest_entries = fs::read_dir(temp_dest.path()).unwrap().count();
    assert_eq!(dest_entries, 0, "Destination directory should remain empty on preflight failure");
}

#[test]
fn test_restore_store_mode_e2e_roundtrip() {
    let temp_src = tempdir().unwrap();
    let temp_parts = tempdir().unwrap();
    let temp_dest = tempdir().unwrap();

    // Create nested directory structure with multiple files
    let sub_a = temp_src.path().join("sub_a");
    let sub_b = temp_src.path().join("sub_b/nested");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();

    fs::write(temp_src.path().join("root.txt"), b"Root file content").unwrap();
    fs::write(sub_a.join("alpha.log"), b"Alpha log data line 1\nline 2").unwrap();
    fs::write(sub_b.join("blob.bin"), vec![0x55; 150_000]).unwrap();

    let chunk_size = 65_536; // 64KB
    let pack_res = pack_archive(
        temp_src.path(),
        temp_parts.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    let manifest = Manifest::new(
        "roundtrip_store".to_string(),
        "directory".to_string(),
        pack_res.total_uncompressed_bytes,
        pack_res.total_entries,
        "store".to_string(),
        chunk_size,
        pack_res.parts,
    );
    manifest
        .save_to_file(&temp_parts.path().join("manifest.json"))
        .unwrap();

    let res = extract_archive(
        temp_parts.path(),
        temp_dest.path(),
        &RestoreOptions::default(),
    )
    .unwrap();

    assert_eq!(res.source_name, "roundtrip_store");
    assert_dirs_equal(temp_src.path(), temp_dest.path());
}

#[test]
fn test_restore_fast_zstd_mode_e2e_roundtrip() {
    let temp_src = tempdir().unwrap();
    let temp_parts = tempdir().unwrap();
    let temp_dest = tempdir().unwrap();

    // Create nested directory structure with compressible and incompressible data
    let sub = temp_src.path().join("deep/dir");
    fs::create_dir_all(&sub).unwrap();

    fs::write(
        temp_src.path().join("manifest_source.json"),
        b"{\"test\": true}".repeat(500),
    )
    .unwrap();
    fs::write(sub.join("large_binary.dat"), vec![0xEE; 200_000]).unwrap();

    let chunk_size = 65_536; // 64KB
    let pack_res = pack_archive(
        temp_src.path(),
        temp_parts.path(),
        chunk_size,
        CompressionMode::Fast,
        false,
    )
    .unwrap();

    let manifest = Manifest::new(
        "roundtrip_fast".to_string(),
        "directory".to_string(),
        pack_res.total_uncompressed_bytes,
        pack_res.total_entries,
        "fast".to_string(),
        chunk_size,
        pack_res.parts,
    );
    manifest
        .save_to_file(&temp_parts.path().join("manifest.json"))
        .unwrap();

    let res = extract_archive(
        temp_parts.path(),
        temp_dest.path(),
        &RestoreOptions::default(),
    )
    .unwrap();

    assert_eq!(res.source_name, "roundtrip_fast");
    assert_dirs_equal(temp_src.path(), temp_dest.path());
}

#[test]
fn test_restore_scripts_generated_and_restore_sh_executable() {
    let temp_src = tempdir().unwrap();
    let temp_parts = tempdir().unwrap();
    let temp_dest = tempdir().unwrap();

    let file_path = temp_src.path().join("readme.txt");
    let content = b"POSIX restore script verification test content!";
    fs::write(&file_path, content).unwrap();

    let chunk_size = 65_536;
    let pack_res = pack_archive(
        temp_src.path(),
        temp_parts.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    let manifest = Manifest::new(
        "script_test".to_string(),
        "directory".to_string(),
        pack_res.total_uncompressed_bytes,
        pack_res.total_entries,
        "store".to_string(),
        chunk_size,
        pack_res.parts,
    );
    let manifest_path = temp_parts.path().join("manifest.json");
    manifest.save_to_file(&manifest_path).unwrap();

    write_restore_scripts(temp_parts.path(), &manifest).unwrap();

    let sh_path = temp_parts.path().join("restore.sh");
    let bat_path = temp_parts.path().join("restore.bat");
    let ps1_path = temp_parts.path().join("restore.ps1");

    assert!(sh_path.exists(), "restore.sh must exist");
    assert!(bat_path.exists(), "restore.bat must exist");
    assert!(ps1_path.exists(), "restore.ps1 must exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&sh_path).unwrap();
        let mode = meta.permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "restore.sh must be executable");

        // Execute restore.sh directly
        let output = Command::new(&sh_path)
            .arg(temp_dest.path())
            .output()
            .expect("Failed to execute restore.sh");

        assert!(
            output.status.success(),
            "restore.sh failed with stdout: {}, stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert_dirs_equal(temp_src.path(), temp_dest.path());
    }
}

#[test]
fn test_restore_checksum_mismatch_fails_and_skip_verify_bypasses() {
    let temp_src = tempdir().unwrap();
    let temp_parts = tempdir().unwrap();

    let file_path = temp_src.path().join("corrupt_test.bin");
    let content = vec![0x11; 5_000];
    fs::write(&file_path, &content).unwrap();

    let chunk_size = 3_000;
    let pack_res = pack_archive(
        temp_src.path(),
        temp_parts.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .unwrap();

    let manifest = Manifest::new(
        "checksum_test".to_string(),
        "directory".to_string(),
        pack_res.total_uncompressed_bytes,
        pack_res.total_entries,
        "store".to_string(),
        chunk_size,
        pack_res.parts.clone(),
    );
    manifest
        .save_to_file(&temp_parts.path().join("manifest.json"))
        .unwrap();

    // Corrupt byte 1500 of data.part001 (inside tar payload)
    let part1 = temp_parts.path().join(&pack_res.parts[0].filename);
    let mut bytes = fs::read(&part1).unwrap();
    bytes[1500] ^= 0xFF;
    fs::write(&part1, &bytes).unwrap();

    // Verification fails
    let dest_verify = tempdir().unwrap();
    let res_verify = extract_archive(
        temp_parts.path(),
        dest_verify.path(),
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

    // Skip verification succeeds
    let dest_skip = tempdir().unwrap();
    let res_skip = extract_archive(
        temp_parts.path(),
        dest_skip.path(),
        &RestoreOptions {
            skip_verify: true,
            verbose: false,
        },
    );
    assert!(res_skip.is_ok(), "Restore with skip_verify: true should bypass checksum check");
}
