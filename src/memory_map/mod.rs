//! Memory map types and entry point for parsing `memory.x` linker scripts.
//!
//! The public API is intentionally narrow:
//! - [`Region`] — a single named memory region with start address and length
//! - [`Map`] — an ordered list of regions
//! - [`from_memory_x`] — parse a `memory.x` file into a [`Map`]
//!
//! Parsing is handled by the `parser` sub-module, which in turn uses
//! `expression` for evaluating `ORIGIN`/`LENGTH` expressions and
//! `tokenizer` for splitting those expressions into tokens.

use std::error::Error;
use std::path::Path;

mod expression;
mod parser;
mod tokenizer;

/// A list of memory regions parsed from a `memory.x` linker script.
pub type Map = Vec<Region>;

/// A single named memory region from a `memory.x` `MEMORY { … }` block.
///
/// Examples: `RAM`, `FLASH`, `dram2_seg`
pub struct Region {
    /// Region name as it appears in the linker script.
    pub name: String,
    /// Start address (`ORIGIN`).
    pub start: u64,
    /// Length in bytes (`LENGTH`).
    pub length: u64,
}

impl Region {
    pub fn ends_at(&self) -> u64 {
        self.start.saturating_add(self.length)
    }
}

/// Parse a `memory.x` linker script file into a [`Map`].
pub fn from_memory_x(memory_x_file_path: &Path) -> Result<Map, Box<dyn Error>> {
    let source = std::fs::read_to_string(memory_x_file_path)?;
    parser::parse(&source)
}
