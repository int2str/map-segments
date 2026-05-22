//! # cargo-map-segments
//!
//! A Cargo subcommand that visualises how ELF binary sections are laid out across
//! the memory regions defined in a linker script (`memory.x`).
//!
//! ## Usage
//!
//! ```bash
//! cargo map-segments
//! cargo map-segments --bin my-app
//! cargo map-segments --bin my-app --release
//! cargo map-segments -m path-to/memory.x
//! ```
//!
//! See the [README](../README.md) for details.

use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use cargo_metadata::Message;
use clap::Parser;
use terminal_size::{Width, terminal_size};

mod memory_map;
mod sections;

use sections::SectionInfo;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    /// Path to an ELF binary to analyse directly. Skips `cargo build` entirely.
    #[arg(short = 'b', long, conflicts_with_all = ["bin", "example", "release"])]
    binary: Option<PathBuf>,

    /// Binary target to build and analyse. If neither --bin nor --example is
    /// given, the first executable artifact produced by `cargo build` is used.
    #[arg(long, conflicts_with = "example")]
    bin: Option<String>,

    /// Example target to build and analyse.
    #[arg(long, conflicts_with = "bin")]
    example: Option<String>,

    /// Build in release mode (default: debug).
    #[arg(long)]
    release: bool,

    /// Path to a memory.x linker script. If omitted, the sibling <binary>.d
    /// dependency file is parsed to locate memory.x automatically.
    #[arg(short = 'm', long)]
    memory_map: Option<PathBuf>,

    /// Output width in columns. Defaults to the current terminal width, or 120
    /// if stdout is not a terminal (e.g. redirected to a file).
    #[arg(short, long)]
    width: Option<usize>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // When invoked as `cargo map-segments`, Cargo inserts "map-segments" as
    // argv[1]. Skip it so Clap sees the same args whether called directly or
    // via cargo.
    let args = std::env::args_os().enumerate().filter_map(|(i, arg)| {
        if i == 1 && arg == "map-segments" {
            None
        } else {
            Some(arg)
        }
    });
    let arguments = Arguments::parse_from(args);

    let elf_path = match arguments.binary {
        Some(path) => path,
        None => build_and_find_elf(&arguments)?,
    };

    let map_path = arguments
        .memory_map
        .or_else(|| find_memory_x_via_fingerprint(&elf_path));

    let map_path = map_path.ok_or(
        "No memory.x found. A <binary>.d dependency file was not found or contained no \
         memory.x entry. Use --memory-map <PATH> to specify one explicitly.",
    )?;

    let sections = sections::from_object_file(&elf_path)?;
    let memory_map = memory_map::from_memory_x(&map_path)?;
    let width = resolve_width(arguments.width);
    map_sections(&sections, &memory_map, width);

    Ok(())
}

/// Run `cargo build --message-format=json` and return the path to the produced
/// executable artifact. Stderr (build progress) is forwarded to the terminal.
fn build_and_find_elf(arguments: &Arguments) -> Result<PathBuf, Box<dyn Error>> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--message-format=json"]);

    if let Some(bin) = &arguments.bin {
        cmd.args(["--bin", bin]);
    } else if let Some(example) = &arguments.example {
        cmd.args(["--example", example]);
    }

    if arguments.release {
        cmd.arg("--release");
    }

    cmd.stdout(Stdio::piped());
    // Forward build progress / errors to the terminal.
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");

    let mut elf_path: Option<PathBuf> = None;
    let mut artifact_count = 0usize;

    for message in Message::parse_stream(std::io::BufReader::new(stdout)) {
        if let Message::CompilerArtifact(artifact) = message?
            && let Some(executable) = artifact.executable
        {
            artifact_count += 1;
            if elf_path.is_none() {
                elf_path = Some(executable.into());
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err("cargo build failed".into());
    }

    if artifact_count > 1 && arguments.bin.is_none() && arguments.example.is_none() {
        eprintln!(
            "Warning: multiple executable artifacts found; using the first one. \
             Use --bin <NAME> to select a specific target."
        );
    }

    elf_path.ok_or_else(|| "cargo build produced no executable artifact".into())
}

fn find_memory_x_via_fingerprint(elf_path: &std::path::Path) -> Option<PathBuf> {
    // Derive the profile directory and binary name from the ELF path.
    // Expected layout: <target_dir>/<triple>/<profile>/<name>
    let binary_name = elf_path.file_name()?.to_str()?;
    let binary_directory = elf_path.parent()?;
    let fingerprint_directory = binary_directory.join(".fingerprint");
    let build_directory = binary_directory.join("build");

    // Step 1: find the fingerprint dir containing dep-bin-<name>.
    // The dir is named <package_name>-{HASH}, which may differ from the binary name.
    let dep_bin_name = format!("dep-bin-{}", binary_name);
    let fingerprint_subdirectory = std::fs::read_dir(&fingerprint_directory)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let dep_bin = entry.path().join(&dep_bin_name);
            if dep_bin.exists() {
                entry.file_name().to_str().map(str::to_string)
            } else {
                None
            }
        })?;

    // Step 2: parse the fingerprint JSON to extract the build_script_build hash X.
    // The JSON file is named bin-<name>.json inside the fingerprint dir.
    let json_path = fingerprint_directory
        .join(&fingerprint_subdirectory)
        .join(format!("bin-{}.json", binary_name));
    let json_str = std::fs::read_to_string(&json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let build_script_hash_value: u64 = json["deps"]
        .as_array()?
        .iter()
        .find(|dep| dep.get(1).and_then(|v| v.as_str()) == Some("build_script_build"))?
        .get(3)?
        .as_u64()?;

    // Step 3: find the run-build-script file whose hex-encoded LE u64 equals X.
    // No idea why this needs to be byte swapped ...
    let target_hex = format!("{:x}", build_script_hash_value.to_be());

    let build_script_hash = std::fs::read_dir(&fingerprint_directory)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let run_file = entry.path().join("run-build-script-build-script-build");
            if !run_file.exists() {
                return None;
            }
            let contents = std::fs::read_to_string(&run_file).ok()?;
            if contents.trim() == target_hex {
                entry
                    .file_name()
                    .to_str()?
                    .split('-')
                    .next_back()
                    .map(str::to_string)
            } else {
                None
            }
        })?;

    // Recover the full directory name for the build script fingerprint.
    // We need <crate-name>-{BS_HASH} but the crate name may differ from the binary name.
    // Re-scan to get the full dir name matching the bs_hash suffix.
    let build_script_directory = std::fs::read_dir(&fingerprint_directory)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let name_str = entry.file_name().to_str()?.to_string();
            if name_str.ends_with(&format!("-{}", build_script_hash)) {
                Some(name_str)
            } else {
                None
            }
        })?;

    // Step 4: read root-output to get OUT_DIR.
    let root_output_path = build_directory
        .join(&build_script_directory)
        .join("root-output");
    let out_dir = std::fs::read_to_string(&root_output_path)
        .ok()
        .map(|s| PathBuf::from(s.trim()))?;

    // Step 5: return OUT_DIR/memory.x if it exists.
    let memory_x = out_dir.join("memory.x");
    if memory_x.exists() {
        Some(memory_x)
    } else {
        None
    }
}

fn resolve_width(override_width: Option<usize>) -> usize {
    override_width
        .or_else(|| terminal_size().map(|(Width(w), _)| w as usize))
        .unwrap_or(120)
}

fn map_regions<'a>(
    sections: &'a [SectionInfo],
    map: &'a memory_map::Map,
) -> Vec<(&'a SectionInfo, &'a memory_map::Region)> {
    let mut results: Vec<(&'a SectionInfo, &'a memory_map::Region)> = Vec::new();
    for section in sections {
        let region = map.iter().rfind(|region| {
            let region_start = region.start;
            let region_end = region.start + region.length;
            region_start <= section.address && region_end >= (section.address + section.size)
        });
        if let Some(region) = &region {
            results.push((section, region));
        }
    }
    results
}

fn longest_names(region_map: &[(&SectionInfo, &memory_map::Region)]) -> (usize, usize) {
    region_map.iter().fold((0, 0), |(section, region), (s, r)| {
        (section.max(s.name.len()), region.max(r.name.len()))
    })
}

fn map_sections(sections: &[SectionInfo], map: &memory_map::Map, width: usize) {
    let region_map = map_regions(sections, map);
    let (section_name_width, region_name_width) = longest_names(&region_map);
    let bar_width = width
        .saturating_sub(section_name_width + region_name_width + 21)
        .max(1);
    let mut last_region: Option<u64> = None;

    for (section, region) in region_map {
        if last_region != Some(region.id) {
            if last_region.is_some() {
                println!();
            }
            last_region = Some(region.id);
        }

        print!(
            "{:width$} {:08x} {:7}",
            section.name,
            section.address,
            section.size,
            width = section_name_width,
        );

        print!(" {:width$} ", region.name, width = region_name_width);
        print_memory(
            region.start,
            region.start + region.length,
            section.address,
            section.size,
            bar_width,
        );

        println!();
    }
}

fn map_range(value: u64, input_range: u64, output_range: usize) -> usize {
    ((output_range as f64 / input_range as f64) * value as f64) as usize
}

fn print_memory(
    region_start: u64,
    region_end: u64,
    block_start: u64,
    block_size: u64,
    total_width: usize,
) {
    let region_size = region_end - region_start;
    let offset = map_range(block_start - region_start, region_size, total_width);
    let width = map_range(block_size, region_size, total_width);

    let block = if width == 0 { "\u{258f}" } else { "\u{2588}" };

    print!(
        "[{}{}{}]",
        " ".repeat(offset),
        block.repeat(width.max(1)),
        " ".repeat(total_width - offset - width.max(1)),
    );
}
