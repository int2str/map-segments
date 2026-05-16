use std::error::Error;
use std::path::Path;

use regex::Regex;

pub struct Map {
    pub regions: Vec<Region>,
}

pub struct Region {
    pub name: String,
    pub id: u64,
    pub start: u64,
    pub length: u64,
}

pub fn from_memory_x(path: &Path) -> Result<Map, Box<dyn Error>> {
    let source = std::fs::read_to_string(path)?;
    let stripped = strip_comments(&source);
    let block = extract_memory_block(&stripped)?;
    let regions = parse_regions(&block)?;
    Ok(Map { regions })
}

// ---------------------------------------------------------------------------
// Comment stripping
// ---------------------------------------------------------------------------

fn strip_comments(input: &str) -> String {
    // Match block comments (non-greedy) or line comments, replace with whitespace.
    let re = Regex::new(r"(?s)/\*.*?\*/|//[^\n]*").unwrap();
    re.replace_all(input, " ").into_owned()
}

// ---------------------------------------------------------------------------
// MEMORY { } block extraction
// ---------------------------------------------------------------------------

fn extract_memory_block(input: &str) -> Result<String, Box<dyn Error>> {
    let m = Regex::new(r"(?i)MEMORY\s*\{")
        .unwrap()
        .find(input)
        .ok_or("No MEMORY block found in linker script")?;

    let content = &input[m.end()..];

    // Find the matching closing brace via depth tracking (regex can't do this).
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

// ---------------------------------------------------------------------------
// Region line parsing
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Expression evaluator
// ---------------------------------------------------------------------------
//
// Supports:
//   - Hex literals:     0x1234ABCD
//   - Decimal literals: 12345
//   - K/M suffixes:     16K  1024K  2M
//   - ORIGIN(NAME) / LENGTH(NAME) references
//   - Binary + and - operators (left-to-right, no precedence needed)

fn evaluate_expression(expr: &str, resolved: &[Region]) -> Result<u64, Box<dyn Error>> {
    // Tokenise into atoms and operators
    let tokens = tokenise(expr)?;
    if tokens.is_empty() {
        return Err(format!("Empty expression: '{expr}'").into());
    }

    // Evaluate left-to-right: value (op value)*
    let mut iter = tokens.into_iter();
    let first = iter.next().unwrap();
    let mut acc = evaluate_atom(&first, resolved)?;

    while let Some(op) = iter.next() {
        let operand_tok = iter
            .next()
            .ok_or_else(|| format!("Missing operand after operator in '{expr}'"))?;
        let operand = evaluate_atom(&operand_tok, resolved)?;
        match op.as_str() {
            "+" => acc = acc.wrapping_add(operand),
            "-" => acc = acc.wrapping_sub(operand),
            other => return Err(format!("Unknown operator '{other}' in '{expr}'").into()),
        }
    }

    Ok(acc)
}

#[derive(Debug)]
enum Token {
    Atom(String),
    Op(char),
}

impl Token {
    fn as_str(&self) -> &str {
        match self {
            Token::Atom(s) => s.as_str(),
            Token::Op(c) => match c {
                '+' => "+",
                '-' => "-",
                _ => "",
            },
        }
    }
}

/// Split expression into alternating atom / operator tokens.
fn tokenise(expr: &str) -> Result<Vec<Token>, Box<dyn Error>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;

    for ch in expr.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                if paren_depth == 0 {
                    return Err(format!("Unmatched ')' in expression '{expr}'").into());
                }
                paren_depth -= 1;
                current.push(ch);
            }
            '+' | '-' if paren_depth == 0 => {
                let atom = current.trim().to_string();
                current = String::new();
                if !atom.is_empty() {
                    tokens.push(Token::Atom(atom));
                }
                tokens.push(Token::Op(ch));
            }
            _ => current.push(ch),
        }
    }

    let tail = current.trim().to_string();
    if !tail.is_empty() {
        tokens.push(Token::Atom(tail));
    }

    Ok(tokens)
}

fn evaluate_atom(token: &Token, resolved: &[Region]) -> Result<u64, Box<dyn Error>> {
    let s = token.as_str().trim();

    // Parenthesised sub-expression: ( ... )
    if s.starts_with('(') && s.ends_with(')') {
        return evaluate_expression(&s[1..s.len() - 1], resolved);
    }

    // ORIGIN(NAME) or LENGTH(NAME)
    if let Some(caps) = Regex::new(r"(?i)^(ORIGIN|LENGTH)\(([A-Za-z_]\w*)\)$")
        .unwrap()
        .captures(s)
    {
        let name = &caps[2];
        return match caps[1].to_ascii_uppercase().as_str() {
            "ORIGIN" => lookup(name, resolved, |r| r.start),
            _ => lookup(name, resolved, |r| r.length),
        };
    }

    parse_literal(s)
}

fn lookup(
    name: &str,
    resolved: &[Region],
    field: fn(&Region) -> u64,
) -> Result<u64, Box<dyn Error>> {
    resolved
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .map(field)
        .ok_or_else(|| format!("Reference to unknown region '{name}'").into())
}

fn parse_literal(s: &str) -> Result<u64, Box<dyn Error>> {
    let re = Regex::new(r"(?i)^(0x[0-9a-f]+|\d+)\s*([km]?)$").unwrap();
    let caps = re
        .captures(s.trim())
        .ok_or_else(|| format!("Invalid numeric literal: '{s}'"))?;

    let n = {
        let raw = &caps[1];
        if raw.starts_with("0x") || raw.starts_with("0X") {
            u64::from_str_radix(&raw[2..], 16)?
        } else {
            raw.parse::<u64>()?
        }
    };

    Ok(match caps[2].to_ascii_lowercase().as_str() {
        "k" => n * 1024,
        "m" => n * 1024 * 1024,
        _ => n,
    })
}
