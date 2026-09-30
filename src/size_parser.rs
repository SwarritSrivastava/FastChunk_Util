pub const MIN_CHUNK_SIZE: u64 = 65_536; // 64 KB

/// Parses a human-readable size string (e.g., "14G", "14GB", "4000MB", "500M", "1GiB") into bytes.
/// Ensures the size is at least 64KB (65,536 bytes) to prevent pathological chunk thrashing.
pub fn parse_human_size(input: &str) -> Result<u64, String> {
    let bytes = parse_size::parse_size(input)
        .map_err(|e| format!("Invalid size format '{}': {}", input, e))?;

    if bytes < MIN_CHUNK_SIZE {
        return Err(format!(
            "Chunk size must be at least {} bytes (64KB), but got {} bytes",
            MIN_CHUNK_SIZE, bytes
        ));
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_units() {
        assert_eq!(parse_human_size("500M").unwrap(), 500 * 1000 * 1000);
        assert_eq!(parse_human_size("500MB").unwrap(), 500 * 1000 * 1000);
        assert_eq!(parse_human_size("500MiB").unwrap(), 500 * 1024 * 1024);
        assert_eq!(parse_human_size("4000MB").unwrap(), 4000 * 1000 * 1000);
        assert_eq!(parse_human_size("14G").unwrap(), 14 * 1000 * 1000 * 1000);
        assert_eq!(parse_human_size("14GB").unwrap(), 14 * 1000 * 1000 * 1000);
        assert_eq!(parse_human_size("1GiB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_human_size("64KiB").unwrap(), 65536);
        assert_eq!(parse_human_size("65536").unwrap(), 65536);
    }

    #[test]
    fn test_underflow_and_invalid_inputs() {
        // Below minimum allowed size
        assert!(parse_human_size("0").is_err());
        assert!(parse_human_size("10B").is_err());
        assert!(parse_human_size("1000").is_err());
        assert!(parse_human_size("65535").is_err());

        // Invalid format / negative
        assert!(parse_human_size("-5M").is_err());
        assert!(parse_human_size("abc").is_err());
        assert!(parse_human_size("").is_err());
    }
}
