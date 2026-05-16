use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use object::{Object, ObjectSection, SectionKind};
use terminal_size::{Width, terminal_size};

#[derive(Debug)]
struct SectionInfo {
    name: String,
    address: u64,
    size: u64,
}

mod memory_map;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    binary_file: PathBuf,

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
    let arguments = Arguments::parse();

    let map_path = arguments
        .memory_map
        .or_else(|| find_memory_x_via_dep_file(&arguments.binary_file));

    let map_path = map_path.ok_or(
        "No memory.x found. A <binary>.d dependency file was not found or contained no \
         memory.x entry. Use --memory-map <PATH> to specify one explicitly.",
    )?;

    let sections = read_sections(arguments.binary_file)?;
    let memory_map = memory_map::from_memory_x(&map_path)?;
    let width = resolve_width(arguments.width);
    map_sections(&sections, &memory_map, width);

    Ok(())
}

/// Locate memory.x by parsing the Cargo-generated `<binary>.d` dependency file
/// that sits alongside the ELF binary. Returns `None` if the .d file is absent,
/// unreadable, or contains no unambiguous memory.x reference.
fn resolve_width(override_width: Option<usize>) -> usize {
    override_width
        .or_else(|| terminal_size().map(|(Width(w), _)| w as usize))
        .unwrap_or(120)
}

fn find_memory_x_via_dep_file(elf: &std::path::Path) -> Option<PathBuf> {
    // <dir>/<name>.d lives next to the ELF binary.
    let dep_file = elf.with_extension("d");
    let content = std::fs::read_to_string(&dep_file).ok()?;

    // .d format:  target: dep1 dep2 \
    //               dep3 dep4
    // Join backslash-continued lines, strip the "target:" prefix, split on whitespace.
    let joined = content
        .lines()
        .map(|l| l.trim_end_matches('\\').trim())
        .collect::<Vec<_>>()
        .join(" ");

    let deps = match joined.find(':') {
        Some(pos) => &joined[pos + 1..],
        None => return None,
    };

    let matches: Vec<PathBuf> = deps
        .split_whitespace()
        .filter(|token| token.ends_with("memory.x"))
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();

    match matches.len() {
        1 => Some(matches.into_iter().next().unwrap()),
        0 => None,
        _ => {
            eprintln!(
                "Multiple memory.x files found in {}: {}. \
                 Use --memory-map to specify which one to use.",
                dep_file.display(),
                matches
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            None
        }
    }
}

fn read_sections(filename: PathBuf) -> Result<Vec<SectionInfo>, Box<dyn Error>> {
    let binary_file = std::fs::read(filename)?;
    let object_file = object::File::parse(&*binary_file)?;

    let mut sections: Vec<SectionInfo> = object_file
        .sections()
        .filter(|section| section.size() != 0)
        .filter(|section| {
            matches!(
                section.kind(),
                SectionKind::Text
                    | SectionKind::Data
                    | SectionKind::ReadOnlyData
                    | SectionKind::ReadOnlyString
                    | SectionKind::UninitializedData
                    | SectionKind::Common
                    | SectionKind::Tls
                    | SectionKind::UninitializedTls
            )
        })
        .map(|section| SectionInfo {
            name: section.name().unwrap_or("").to_string(),
            address: section.address(),
            size: section.size(),
        })
        .collect();
    sections.sort_by_key(|s| s.address);
    Ok(sections)
}

fn longest_section_name(sections: &[SectionInfo]) -> usize {
    sections
        .iter()
        .map(|section| section.name.len())
        .max()
        .unwrap_or(0)
}

fn map_sections(sections: &[SectionInfo], map: &memory_map::Map, width: usize) {
    let section_name_max_width = longest_section_name(sections);
    let bar_width = width.saturating_sub(section_name_max_width + 30).max(1);
    let mut last_region: Option<u64> = None;

    for section in sections {
        let region = map.regions.iter().find(|region| {
            let region_start = region.start;
            let region_end = region.start + region.length;
            region_start <= section.address && region_end >= (section.address + section.size)
        });

        if let Some(region) = &region
            && last_region != Some(region.id)
        {
            println!();
            last_region = Some(region.id);
        }

        print!(
            "{:width$} {:08x} {:7}",
            section.name,
            section.address,
            section.size,
            width = section_name_max_width,
        );

        if let Some(region) = &region {
            print!(" {:8} ", region.name);
            print_memory(
                region.start,
                region.start + region.length,
                section.address,
                section.size,
                bar_width,
            );
        }

        println!();
    }
}

fn print_memory(
    region_start: u64,
    region_end: u64,
    block_start: u64,
    block_size: u64,
    width: usize,
) {
    let region_size = region_end - region_start;
    let offset =
        ((width as f64 / region_size as f64) * (block_start as f64 - region_start as f64)) as usize;
    let w = ((width as f64 / region_size as f64) * block_size as f64) as usize;

    let small = w == 0;
    let w = w.max(1);

    print!("[");

    for _ in 0..offset {
        print!(" ");
    }
    for _ in 0..w {
        if small {
            print!("\u{258f}");
        } else {
            print!("\u{2588}");
        }
    }
    for _ in 0..(width - w - offset) {
        print!(" ");
    }
    print!("]");
}
