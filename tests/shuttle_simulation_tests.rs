use std::fs;
use tempfile::tempdir;

use fastchunk::archiver::pack_archive;
use fastchunk::cli::CompressionMode;
use fastchunk::manifest::Manifest;
use fastchunk::restore::{extract_archive, verify_parts_exist, RestoreError, RestoreOptions};

#[test]
fn test_single_pen_drive_shuttle_simulation_workflow() {
    // 1. Setup Source Directory (Simulating a multi-file folder to transfer)
    let src_dir = tempdir().expect("failed to create src dir");
    let staging_dir = tempdir().expect("failed to create staging dir");
    let pen_drive_dir = tempdir().expect("failed to create pen drive dir");
    let target_dest_dir = tempdir().expect("failed to create target dest dir");
    let unpacked_output_dir = tempdir().expect("failed to create unpack dest dir");

    // Populate source directory with non-trivial byte patterns so compressed stream spans multiple parts
    let mut file1_data = vec![0u8; 1024 * 1024]; // 1 MB
    for (i, b) in file1_data.iter_mut().enumerate() {
        *b = ((i * 1103515245 + 12345) & 0xFF) as u8;
    }
    let mut file2_data = vec![0u8; 1024 * 1024]; // 1 MB
    for (i, b) in file2_data.iter_mut().enumerate() {
        *b = ((i * 214013 + 2531011) & 0xFF) as u8;
    }
    let mut file3_data = vec![0u8; 512 * 1024];  // 512 KB
    for (i, b) in file3_data.iter_mut().enumerate() {
        *b = ((i * 69069 + 1) & 0xFF) as u8;
    }

    let sub = src_dir.path().join("subfolder");
    fs::create_dir_all(&sub).unwrap();

    fs::write(src_dir.path().join("large_game_file.bin"), &file1_data).unwrap();
    fs::write(sub.join("video_clip.mp4"), &file2_data).unwrap();
    fs::write(sub.join("notes.txt"), &file3_data).unwrap();

    // 2. Split with FastChunk in Store mode into 256KB chunks (guaranteeing ~10 parts)
    let chunk_size: u64 = 256 * 1024;
    let pack_result = pack_archive(
        src_dir.path(),
        staging_dir.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .expect("packing failed");

    let manifest = Manifest::new(
        "source_data".to_string(),
        "directory".to_string(),
        pack_result.total_uncompressed_bytes,
        pack_result.total_entries,
        "store".to_string(),
        chunk_size,
        pack_result.parts.clone(),
    );
    let manifest_path = staging_dir.path().join("manifest.json");
    manifest.save_to_file(&manifest_path).unwrap();
    fastchunk::scripts::write_restore_scripts(staging_dir.path(), &manifest).unwrap();

    let total_parts = pack_result.parts.len();
    assert!(total_parts >= 3, "Expected at least 3 parts for testing shuttle");

    // 3. Shuttle initial parts through pen drive (simulating copying to pen drive, taking to destination)
    // Trip 1: Copy manifest, scripts, and first 2 parts to pen drive, then offload to target destination
    fs::copy(&manifest_path, pen_drive_dir.path().join("manifest.json")).unwrap();
    fs::copy(
        staging_dir.path().join("restore.sh"),
        pen_drive_dir.path().join("restore.sh"),
    )
    .unwrap();

    // Move Trip 1 from pen drive to destination
    fs::copy(
        pen_drive_dir.path().join("manifest.json"),
        target_dest_dir.path().join("manifest.json"),
    )
    .unwrap();
    fs::copy(
        pen_drive_dir.path().join("restore.sh"),
        target_dest_dir.path().join("restore.sh"),
    )
    .unwrap();

    // Copy parts 1 and 2
    for i in 0..2 {
        let part_name = &pack_result.parts[i].filename;
        let p_src = staging_dir.path().join(part_name);
        let p_drive = pen_drive_dir.path().join(part_name);
        let p_dest = target_dest_dir.path().join(part_name);

        fs::copy(&p_src, &p_drive).unwrap();
        fs::copy(&p_drive, &p_dest).unwrap();
        // Clear pen drive for next trip
        fs::remove_file(&p_drive).unwrap();
    }

    // 4. Test Pre-Flight Missing-Part Detection:
    // Destination only has part 1 & part 2, but needs `total_parts`.
    // Attempting to restore MUST fail early and explicitly identify missing parts.
    let preflight_err = verify_parts_exist(target_dest_dir.path(), &manifest)
        .expect_err("Should fail when parts are missing");

    match preflight_err {
        RestoreError::MissingParts(missing) => {
            assert_eq!(missing.len(), total_parts - 2);
            assert!(missing[0].contains(&pack_result.parts[2].filename));
        }
        other => panic!("Expected MissingParts error, got: {:?}", other),
    }

    // Extraction should also fail without modifying destination
    let extract_err = extract_archive(
        target_dest_dir.path(),
        unpacked_output_dir.path(),
        &RestoreOptions {
            skip_verify: false,
            verbose: false,
        },
    )
    .expect_err("Extract must abort when parts are missing");

    match extract_err {
        RestoreError::MissingParts(_) => {}
        other => panic!("Expected MissingParts, got {:?}", other),
    }

    // 5. Shuttle remaining parts one by one via simulated pen drive
    for i in 2..total_parts {
        let part_name = &pack_result.parts[i].filename;
        let p_src = staging_dir.path().join(part_name);
        let p_drive = pen_drive_dir.path().join(part_name);
        let p_dest = target_dest_dir.path().join(part_name);

        // Put on pen drive
        fs::copy(&p_src, &p_drive).unwrap();
        // Move from pen drive to destination
        fs::copy(&p_drive, &p_dest).unwrap();
        // Clear pen drive
        fs::remove_file(&p_drive).unwrap();
    }

    // 6. Now all parts exist at destination. Run extraction!
    let restore_result = extract_archive(
        target_dest_dir.path(),
        unpacked_output_dir.path(),
        &RestoreOptions {
            skip_verify: false,
            verbose: false,
        },
    )
    .expect("Restoration failed when all parts were present");

    assert_eq!(
        restore_result.total_bytes_uncompressed,
        pack_result.total_uncompressed_bytes
    );

    // 7. Verify byte-for-byte exact restored files
    let restored_file1 = fs::read(unpacked_output_dir.path().join("large_game_file.bin")).unwrap();
    assert_eq!(restored_file1, file1_data);

    let restored_file2 = fs::read(unpacked_output_dir.path().join("subfolder/video_clip.mp4")).unwrap();
    assert_eq!(restored_file2, file2_data);

    let restored_file3 = fs::read(unpacked_output_dir.path().join("subfolder/notes.txt")).unwrap();
    assert_eq!(restored_file3, file3_data);
}
