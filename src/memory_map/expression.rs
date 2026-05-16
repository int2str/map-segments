//! Expression evaluator for `memory.x` numeric expressions.
//!
//! Evaluates expressions of the form used in `ORIGIN` and `LENGTH` values:
//!
//! - Hex literals (`0x1234ABCD`) and decimal literals
//! - `K` / `M` multiplier suffixes (`16K`, `2M`)
//! - `ORIGIN(NAME)` and `LENGTH(NAME)` references to already-resolved regions
//! - Binary `+` and `-` operators (left-to-right, equal precedence)
//! - Parenthesised sub-expressions (`(ORIGIN(RAM) + LENGTH(RAM))`)

use std::error::Error;

use regex::Regex;

use super::Region;
use super::tokenizer::{Token, tokenise};

/// Evaluate a `memory.x` expression and return its value.
///
/// `resolved` is the list of regions already parsed; it is used to look up
/// `ORIGIN(NAME)` and `LENGTH(NAME)` references.
pub fn evaluate_expression(expression: &str, resolved: &[Region]) -> Result<u64, Box<dyn Error>> {
    let tokens = tokenise(expression)?;
    if tokens.is_empty() {
        return Err(format!("Empty expression: '{expression}'").into());
    }

    // Evaluate left-to-right: value (op value)*
    let mut token_iterator = tokens.into_iter();
    let first_token = token_iterator.next().unwrap();
    let mut result = evaluate_atom(&first_token, resolved)?;

    while let Some(op) = token_iterator.next() {
        let operand_tok = token_iterator
            .next()
            .ok_or_else(|| format!("Missing operand after operator in '{expression}'"))?;
        let operand = evaluate_atom(&operand_tok, resolved)?;
        match op.as_str() {
            "+" => result = result.wrapping_add(operand),
            "-" => result = result.wrapping_sub(operand),
            other => return Err(format!("Unknown operator '{other}' in '{expression}'").into()),
        }
    }

    Ok(result)
}

/// Evaluate a single atom token.
///
/// Handles parenthesised sub-expressions, `ORIGIN(NAME)` / `LENGTH(NAME)`
/// calls, and plain numeric literals.
fn evaluate_atom(token: &Token, resolved: &[Region]) -> Result<u64, Box<dyn Error>> {
    let token_string = token.as_str().trim();

    // Parenthesised sub-expression: ( ... )
    if token_string.starts_with('(') && token_string.ends_with(')') {
        return evaluate_expression(&token_string[1..token_string.len() - 1], resolved);
    }

    // ORIGIN(NAME) or LENGTH(NAME)
    if let Some(groups) = Regex::new(r"(?i)^(ORIGIN|LENGTH)\(([A-Za-z_]\w*)\)$")
        .unwrap()
        .captures(token_string)
    {
        let name = &groups[2];
        return match groups[1].to_ascii_uppercase().as_str() {
            "ORIGIN" => lookup(name, resolved, |r| r.start),
            _ => lookup(name, resolved, |r| r.length),
        };
    }

    parse_literal(token_string)
}

/// Look up a region by name and extract a field value.
fn lookup(
    name: &str,
    resolved: &[Region],
    field: fn(&Region) -> u64,
) -> Result<u64, Box<dyn Error>> {
    resolved
        .iter()
        .find(|region| region.name.eq_ignore_ascii_case(name))
        .map(field)
        .ok_or_else(|| format!("Reference to unknown region '{name}'").into())
}

/// Parse a plain numeric literal with an optional `K` or `M` suffix.
fn parse_literal(s: &str) -> Result<u64, Box<dyn Error>> {
    let re = Regex::new(r"(?i)^(0x[0-9a-f]+|\d+)\s*([km]?)$").unwrap();
    let groups = re
        .captures(s.trim())
        .ok_or_else(|| format!("Invalid numeric literal: '{s}'"))?;

    let n = {
        let raw = &groups[1];
        if raw.starts_with("0x") || raw.starts_with("0X") {
            u64::from_str_radix(&raw[2..], 16)?
        } else {
            raw.parse::<u64>()?
        }
    };

    Ok(match groups[2].to_ascii_lowercase().as_str() {
        "k" => n * 1_024,
        "m" => n * 1_024 * 1_024,
        _ => n,
    })
}
