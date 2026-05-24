use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Path to an ELF binary to analyse directly. Skips `cargo build` entirely.
    #[arg(
        short = 'b',
        long,
        conflicts_with_all = [
            "bin", "example", "release", "features", "all_features",
            "no_default_features", "profile", "target", "package", "manifest_path",
        ]
    )]
    pub binary: Option<PathBuf>,

    /// Binary target to build and analyse. If neither --bin nor --example is
    /// given, the first executable artifact produced by `cargo build` is used.
    #[arg(long, conflicts_with = "example")]
    pub bin: Option<String>,

    /// Example target to build and analyse.
    #[arg(long, conflicts_with = "bin")]
    pub example: Option<String>,

    /// Build in release mode (default: debug).
    #[arg(long)]
    pub release: bool,

    /// Space or comma separated list of features to activate.
    #[arg(short = 'F', long)]
    pub features: Option<String>,

    /// Activate all available features.
    #[arg(long)]
    pub all_features: bool,

    /// Do not activate the `default` feature.
    #[arg(long)]
    pub no_default_features: bool,

    /// Build artifacts with the specified profile.
    #[arg(long)]
    pub profile: Option<String>,

    /// Build for the target triple.
    #[arg(long)]
    pub target: Option<String>,

    /// Package to build.
    #[arg(short = 'p', long)]
    pub package: Option<String>,

    /// Path to Cargo.toml.
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,

    /// Path to a memory.x linker script. If omitted, it is located automatically
    /// via Cargo's fingerprint metadata.
    #[arg(short = 'm', long)]
    pub memory_map: Option<PathBuf>,

    /// Output width in columns. Defaults to the current terminal width, or 120
    /// if stdout is not a terminal (e.g. redirected to a file).
    #[arg(short, long)]
    pub width: Option<usize>,
}

pub fn parse() -> Arguments {
    let arguments = arguments_without_package_name();
    Arguments::parse_from(arguments)
}

fn arguments_without_package_name() -> Vec<std::ffi::OsString> {
    // When invoked as `cargo map-segments`, Cargo inserts "map-segments" as
    // argv[1]. Skip it so Clap sees the same args whether called directly or
    // via cargo.
    std::env::args_os()
        .enumerate()
        .filter_map(|(i, arg)| {
            if i == 1 && arg == "map-segments" {
                None
            } else {
                Some(arg)
            }
        })
        .collect()
}
