use std::fs;
use std::time::Instant;
use tempfile::tempdir;

use fastchunk::archiver::pack_archive;
use fastchunk::cli::CompressionMode;
use fastchunk::manifest::Manifest;
use fastchunk::restore::{extract_archive, RestoreOptions};

#[test]
fn test_extraction_throughput_benchmark() {
    let src_dir = tempdir().expect("failed to create src dir");
    let staging_store = tempdir().expect("failed to create store staging");
    let staging_fast = tempdir().expect("failed to create fast staging");
    let out_store = tempdir().expect("failed to create store out");
    let out_fast = tempdir().expect("failed to create fast out");

    // Generate ~20MB of test payload
    let test_bytes: usize = 20 * 1024 * 1024;
    let payload = vec![0x5A; test_bytes];
    fs::write(src_dir.path().join("payload.dat"), &payload).unwrap();

    let chunk_size = 5 * 1024 * 1024; // 5 MB chunks

    // --- Benchmark 1: Store Mode (Zero-Compression Raw Streaming) ---
    let pack_store = pack_archive(
        src_dir.path(),
        staging_store.path(),
        chunk_size,
        CompressionMode::Store,
        false,
    )
    .expect("Store pack failed");

    let manifest_store = Manifest::new(
        "payload".to_string(),
        "directory".to_string(),
        pack_store.total_uncompressed_bytes,
        pack_store.total_entries,
        "store".to_string(),
        chunk_size,
        pack_store.parts.clone(),
    );
    manifest_store
        .save_to_file(&staging_store.path().join("manifest.json"))
        .unwrap();

    let start_store = Instant::now();
    let res_store = extract_archive(
        staging_store.path(),
        out_store.path(),
        &RestoreOptions {
            skip_verify: true, // test pure I/O stream extraction speed
            verbose: false,
        },
    )
    .expect("Store extract failed");
    let duration_store = start_store.elapsed();

    let store_mb = (res_store.total_bytes_uncompressed as f64) / (1024.0 * 1024.0);
    let store_secs = duration_store.as_secs_f64();
    let store_throughput = store_mb / store_secs.max(0.0001);

    println!(
        "\n[Benchmark Store Mode] Extracted {:.2} MB in {:.4}s -> Throughput: {:.2} MB/s",
        store_mb, store_secs, store_throughput
    );

    assert_eq!(
        fs::read(out_store.path().join("payload.dat")).unwrap(),
        payload
    );

    // --- Benchmark 2: Fast Mode (Zstd Level 1 Streaming) ---
    let pack_fast = pack_archive(
        src_dir.path(),
        staging_fast.path(),
        chunk_size,
        CompressionMode::Fast,
        false,
    )
    .expect("Fast pack failed");

    let manifest_fast = Manifest::new(
        "payload".to_string(),
        "directory".to_string(),
        pack_fast.total_uncompressed_bytes,
        pack_fast.total_entries,
        "fast".to_string(),
        chunk_size,
        pack_fast.parts.clone(),
    );
    manifest_fast
        .save_to_file(&staging_fast.path().join("manifest.json"))
        .unwrap();

    let start_fast = Instant::now();
    let res_fast = extract_archive(
        staging_fast.path(),
        out_fast.path(),
        &RestoreOptions {
            skip_verify: true,
            verbose: false,
        },
    )
    .expect("Fast extract failed");
    let duration_fast = start_fast.elapsed();

    let fast_mb = (res_fast.total_bytes_uncompressed as f64) / (1024.0 * 1024.0);
    let fast_secs = duration_fast.as_secs_f64();
    let fast_throughput = fast_mb / fast_secs.max(0.0001);

    println!(
        "[Benchmark Fast Zstd Mode] Extracted {:.2} MB in {:.4}s -> Throughput: {:.2} MB/s\n",
        fast_mb, fast_secs, fast_throughput
    );

    assert_eq!(
        fs::read(out_fast.path().join("payload.dat")).unwrap(),
        payload
    );

    // Extraction should complete in well under 1 second for 20MB
    assert!(
        store_secs < 2.0,
        "Store extraction took too long: {:.2}s",
        store_secs
    );
    assert!(
        fast_secs < 2.0,
        "Fast extraction took too long: {:.2}s",
        fast_secs
    );
}
