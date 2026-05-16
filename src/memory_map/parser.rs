//! Parser for `memory.x` linker scripts.
//!
//! Extracts the `MEMORY { … }` block from a linker script, then parses each
//! region line into a [`Region`] by evaluating its `ORIGIN` and `LENGTH`
//! expressions.

use std::error::Error;

use regex::Regex;

use super::Region;
use super::expression::evaluate_expression;

/// Parse a `memory.x` source string into a list of [`Region`]s.
pub fn parse(source: &str) -> Result<Vec<Region>, Box<dyn Error>> {
    let stripped = strip_comments(source);
    let block = extract_memory_block(&stripped)?;
    parse_regions(&block)
}

/// Remove C-style block comments (`/* … */`) and line comments (`// …`).
///
/// Comments are replaced with a single space to avoid accidentally joining
/// tokens that were separated only by the comment.
fn strip_comments(input: &str) -> String {
    let re = Regex::new(r"(?s)/\*.*?\*/|//[^\n]*").unwrap();
    re.replace_all(input, " ").into_owned()
}

/// Extract the contents of the `MEMORY { … }` block (without the braces).
///
/// Uses depth-tracking rather than a regex so nested braces (unusual but
/// valid) are handled correctly.
fn extract_memory_block(input: &str) -> Result<String, Box<dyn Error>> {
    let m = Regex::new(r"(?i)MEMORY\s*\{")
        .unwrap()
        .find(input)
        .ok_or("No MEMORY block found in linker script")?;

    let content = &input[m.end()..];

    let mut depth = 1usize;
    let mut end = 0;
    for (i, ch) in content.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err("No closing '}' found for MEMORY block".into());
    }

    Ok(content[..end].to_string())
}

/// Parse region lines from the body of a `MEMORY { … }` block.
///
/// Each line is expected to match:
/// ```text
/// NAME [(attrs)] : ORIGIN = <expr>, LENGTH = <expr>
/// ```
/// Lines that do not match (blank lines, comments already stripped) are
/// silently skipped. Regions are assigned sequential `id` values in file
/// order.
fn parse_regions(block: &str) -> Result<Vec<Region>, Box<dyn Error>> {
    let re = Regex::new(
        r"(?ix)
        ^\s*
        (?P<name>[A-Za-z_]\w*)   # region name
        \s*(?:\([^)]*\))?        # optional (attrs), discarded
        \s*:\s*
        (?:ORIGIN|org)\s*=\s*(?P<origin>[^,]+?)
        \s*,\s*
        (?:LENGTH|len)\s*=\s*(?P<length>.+?)
        \s*$",
    )
    .unwrap();

    let mut regions: Vec<Region> = Vec::new();

    for line in block.lines() {
        if let Some(caps) = re.captures(line) {
            let name = caps["name"].to_string();
            let origin_val = evaluate_expression(caps["origin"].trim(), &regions)?;
            let length_val = evaluate_expression(caps["length"].trim(), &regions)?;
            let id = regions.len() as u64;
            regions.push(Region {
                name,
                id,
                start: origin_val,
                length: length_val,
            });
        }
    }

    Ok(regions)
}
