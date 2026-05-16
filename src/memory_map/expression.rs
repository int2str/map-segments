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
pub(super) fn parse_literal(s: &str) -> Result<u64, Box<dyn Error>> {
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

#[cfg(test)]
mod tests {
    mod parse_literal {
        use super::super::parse_literal;

        // -- Successful cases -----------------------------------------------------

        #[test]
        fn decimal_zero() {
            assert_eq!(parse_literal("0").unwrap(), 0);
        }

        #[test]
        fn decimal_plain() {
            assert_eq!(parse_literal("1234").unwrap(), 1234);
        }

        #[test]
        fn decimal_large() {
            assert_eq!(parse_literal("4294967295").unwrap(), 4_294_967_295);
        }

        #[test]
        fn hex_lowercase_prefix() {
            assert_eq!(parse_literal("0x1000").unwrap(), 0x1000);
        }

        #[test]
        fn hex_uppercase_prefix() {
            assert_eq!(parse_literal("0X1000").unwrap(), 0x1000);
        }

        #[test]
        fn hex_mixed_case_digits() {
            assert_eq!(parse_literal("0xDeAdBeEf").unwrap(), 0xDEAD_BEEF);
        }

        #[test]
        fn hex_zero() {
            assert_eq!(parse_literal("0x0").unwrap(), 0);
        }

        #[test]
        fn hex_large() {
            assert_eq!(parse_literal("0x20000000").unwrap(), 0x2000_0000);
        }

        #[test]
        fn suffix_k_lowercase() {
            assert_eq!(parse_literal("16k").unwrap(), 16 * 1_024);
        }

        #[test]
        fn suffix_k_uppercase() {
            assert_eq!(parse_literal("16K").unwrap(), 16 * 1_024);
        }

        #[test]
        fn suffix_m_lowercase() {
            assert_eq!(parse_literal("2m").unwrap(), 2 * 1_024 * 1_024);
        }

        #[test]
        fn suffix_m_uppercase() {
            assert_eq!(parse_literal("2M").unwrap(), 2 * 1_024 * 1_024);
        }

        #[test]
        fn suffix_k_zero() {
            assert_eq!(parse_literal("0K").unwrap(), 0);
        }

        #[test]
        fn whitespace_leading_trailing() {
            assert_eq!(parse_literal("  256K  ").unwrap(), 256 * 1_024);
        }

        // -- Failure cases --------------------------------------------------------

        #[test]
        fn empty_string() {
            assert!(parse_literal("").is_err());
        }

        #[test]
        fn letters_only() {
            assert!(parse_literal("RAM").is_err());
        }

        #[test]
        fn suffix_without_number() {
            assert!(parse_literal("K").is_err());
        }

        #[test]
        fn hex_invalid_digit() {
            assert!(parse_literal("0xGHIJ").is_err());
        }

        #[test]
        fn float_rejected() {
            assert!(parse_literal("1.5").is_err());
        }

        #[test]
        fn expression_rejected() {
            // Operators are not part of a literal; the tokenizer strips them first.
            assert!(parse_literal("16K + 4").is_err());
        }

        #[test]
        fn hex_prefix_only() {
            assert!(parse_literal("0x").is_err());
        }
    } // mod parse_literal
} // mod tests
