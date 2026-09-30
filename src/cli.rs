use clap::{Parser, Subcommand, ValueEnum};
use std::error::Error;
use std::path::PathBuf;

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

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Split a file or directory into bounded chunk parts
    Split(SplitArgs),
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

pub fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Split(args) => {
            let chunk_size_bytes = args.validate().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            if args.verbose {
                println!(
                    "Splitting '{}' into chunks of {} bytes in '{}' (mode: {})",
                    args.source.display(),
                    chunk_size_bytes,
                    args.output.display(),
                    args.mode
                );
            }
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
        }
    }
}
