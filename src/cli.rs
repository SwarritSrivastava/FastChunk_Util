use clap::{Parser, Subcommand, ValueEnum};
use std::error::Error;
use std::path::PathBuf;

use crate::archiver::pack_archive;
use crate::manifest::Manifest;
use crate::size_parser::parse_human_size;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    #[value(name = "store")]
    Store,
    #[value(name = "fast")]
    Fast,
}

impl std::fmt::Display for CompressionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionMode::Store => write!(f, "store"),
            CompressionMode::Fast => write!(f, "fast"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct SplitArgs {
    /// Path to directory or file to split
    pub source: PathBuf,

    /// Directory where chunks and manifest.json will be written
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Chunk size with units (e.g. 14G, 14GB, 4000MB, 500M, 1GiB)
    #[arg(short = 's', long = "chunk-size")]
    pub chunk_size: String,

    /// Compression mode: 'store' (raw streaming) or 'fast' (zstd level 1)
    #[arg(short = 'm', long = "mode", value_enum, default_value = "fast")]
    pub mode: CompressionMode,

    /// Bundle the current fastchunk executable into the output directory alongside chunks and scripts
    #[arg(short = 'b', long = "bundle-executable", default_value_t = false)]
    pub bundle_executable: bool,

    /// Enable verbose diagnostic logging
    #[arg(short = 'v', long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

impl SplitArgs {
    pub fn validate(&self) -> Result<u64, String> {
        if !self.source.exists() {
            return Err(format!("Source path '{}' does not exist", self.source.display()));
        }
        let chunk_size_bytes = parse_human_size(&self.chunk_size)?;
        Ok(chunk_size_bytes)
    }
}

#[derive(Parser, Debug, Clone)]
pub struct RestoreArgs {
    /// Path to directory containing manifest.json and chunk parts
    pub parts_dir: PathBuf,

    /// Target directory where extracted files will be written
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Skip SHA-256 integrity verification of parts before restoring
    #[arg(long = "skip-verify", default_value_t = false)]
    pub skip_verify: bool,

    /// Enable verbose diagnostic logging
    #[arg(short = 'v', long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Split a file or directory into bounded chunk parts
    Split(SplitArgs),
    /// Restore a file or directory from chunk parts
    Restore(RestoreArgs),
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "fastchunk",
    about = "High-throughput streaming chunking and restorer engine",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

pub fn bundle_current_executable(output_dir: &std::path::Path) -> std::io::Result<PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_name = exe_path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("fastchunk"));
    let target_path = output_dir.join(exe_name);
    std::fs::copy(&exe_path, &target_path)?;
    Ok(target_path)
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Split(args) => {
            let chunk_size_bytes = args
                .validate()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            if args.verbose {
                println!(
                    "Splitting '{}' into chunks of {} bytes in '{}' (mode: {})",
                    args.source.display(),
                    chunk_size_bytes,
                    args.output.display(),
                    args.mode
                );
            }

            let result = pack_archive(
                &args.source,
                &args.output,
                chunk_size_bytes,
                args.mode,
                args.verbose,
            )?;

            let source_name = args
                .source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("archive")
                .to_string();

            let source_type = if args.source.is_dir() {
                "directory".to_string()
            } else {
                "file".to_string()
            };

            let manifest = Manifest::new(
                source_name,
                source_type,
                result.total_uncompressed_bytes,
                result.total_entries,
                args.mode.to_string(),
                chunk_size_bytes,
                result.parts.clone(),
            );

            let manifest_path = args.output.join("manifest.json");
            manifest.save_to_file(&manifest_path)?;

            crate::scripts::write_restore_scripts(&args.output, &manifest)?;

            let bundled_path = if args.bundle_executable {
                match bundle_current_executable(&args.output) {
                    Ok(bundled) => Some(bundled),
                    Err(e) => {
                        if args.verbose {
                            eprintln!("Warning: Failed to bundle executable: {}", e);
                        }
                        None
                    }
                }
            } else {
                None
            };

            println!("Split complete!");
            println!("  Source:          {}", args.source.display());
            println!("  Output:          {}", args.output.display());
            println!("  Mode:            {}", args.mode);
            println!("  Total entries:   {}", result.total_entries);
            println!("  Total raw bytes: {}", result.total_uncompressed_bytes);
            println!("  Parts generated: {}", result.parts.len());
            for part in &result.parts {
                println!("    - {} ({} bytes, sha256: {})", part.filename, part.size, part.sha256);
            }
            println!("  Manifest:        {}", manifest_path.display());
            println!("  Scripts:         restore.sh, restore.bat, restore.ps1 generated");
            if let Some(bundled) = bundled_path {
                println!("  Bundled binary:  {}", bundled.display());
            }
        }
        Commands::Restore(args) => {
            if !args.parts_dir.exists() || !args.parts_dir.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Parts directory '{}' does not exist or is not a directory",
                        args.parts_dir.display()
                    ),
                )
                .into());
            }

            println!(
                "Restoring archive from '{}' to '{}'...",
                args.parts_dir.display(),
                args.output.display()
            );

            let res = crate::restore::extract_archive(
                &args.parts_dir,
                &args.output,
                &crate::restore::RestoreOptions {
                    skip_verify: args.skip_verify,
                    verbose: args.verbose,
                },
            )?;

            println!("Restore complete!");
            println!("  Source archive: {}", res.source_name);
            println!("  Target:         {}", args.output.display());
            println!("  Entries:        {}", res.total_entries_restored);
            println!("  Uncompressed:   {} bytes", res.total_bytes_uncompressed);
            println!("  Time elapsed:   {:.2?}", res.duration);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_split() {
        let cli = Cli::try_parse_from([
            "fastchunk",
            "split",
            "src_dir",
            "-o",
            "out_dir",
            "-s",
            "14G",
            "-m",
            "store",
        ])
        .unwrap();

        match cli.command {
            Commands::Split(args) => {
                assert_eq!(args.source, PathBuf::from("src_dir"));
                assert_eq!(args.output, PathBuf::from("out_dir"));
                assert_eq!(args.chunk_size, "14G");
                assert_eq!(args.mode, CompressionMode::Store);
                assert!(!args.verbose);
            }
            _ => panic!("Expected Split command"),
        }
    }

    #[test]
    fn test_cli_default_mode_is_fast() {
        let cli = Cli::try_parse_from([
            "fastchunk",
            "split",
            "src_dir",
            "-o",
            "out_dir",
            "-s",
            "500M",
        ])
        .unwrap();

        match cli.command {
            Commands::Split(args) => {
                assert_eq!(args.mode, CompressionMode::Fast);
            }
            _ => panic!("Expected Split command"),
        }
    }

    #[test]
    fn test_cli_parse_restore() {
        let cli = Cli::try_parse_from([
            "fastchunk",
            "restore",
            "parts_dir",
            "-o",
            "out_dir",
            "--skip-verify",
        ])
        .unwrap();

        match cli.command {
            Commands::Restore(args) => {
                assert_eq!(args.parts_dir, PathBuf::from("parts_dir"));
                assert_eq!(args.output, PathBuf::from("out_dir"));
                assert!(args.skip_verify);
                assert!(!args.verbose);
            }
            _ => panic!("Expected Restore command"),
        }
    }

    #[test]
    fn test_cli_parse_split_with_bundle_executable() {
        let cli_short = Cli::try_parse_from([
            "fastchunk",
            "split",
            "src_dir",
            "-o",
            "out_dir",
            "-s",
            "1G",
            "-b",
        ])
        .unwrap();

        match cli_short.command {
            Commands::Split(args) => {
                assert!(args.bundle_executable);
            }
            _ => panic!("Expected Split command"),
        }

        let cli_long = Cli::try_parse_from([
            "fastchunk",
            "split",
            "src_dir",
            "-o",
            "out_dir",
            "-s",
            "1G",
            "--bundle-executable",
        ])
        .unwrap();

        match cli_long.command {
            Commands::Split(args) => {
                assert!(args.bundle_executable);
            }
            _ => panic!("Expected Split command"),
        }
    }

    #[test]
    fn test_bundle_executable_copies_binary() {
        let temp = tempfile::tempdir().unwrap();
        let bundled = bundle_current_executable(temp.path()).unwrap();
        assert!(bundled.exists());
        let metadata = std::fs::metadata(&bundled).unwrap();
        assert!(metadata.len() > 0);
    }
}
