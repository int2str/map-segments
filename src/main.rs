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

use terminal_size::{Width, terminal_size};

mod arguments;
mod builder;
mod layout;
mod memory_map;
mod render;
mod sections;

use arguments::Arguments;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = arguments::parse();

    let elf_path = match arguments.binary {
        Some(path) => path,
        None => builder::build_and_find_elf(&arguments)?,
    };

    let map_path = arguments
        .memory_map
        .or_else(|| builder::find_memory_x(&elf_path));

    let map_path = map_path.ok_or(
        "No memory.x found. Could not locate it via Cargo fingerprint metadata. \
         Use --memory-map <PATH> to specify one explicitly.",
    )?;

    let sections = sections::from_object_file(&elf_path)?;
    let memory_map = memory_map::from_memory_x(&map_path)?;
    let region_layouts = layout::map_sections_to_regions(&sections, &memory_map);

    let width = arguments
        .width
        .or_else(|| terminal_size().map(|(Width(w), _)| w as usize))
        .unwrap_or(120);

    if arguments.vertical {
        render::print_vertical_map(&region_layouts, width);
    } else {
        render::print_horizontal_map(&region_layouts, width);
    }
    Ok(())
}
