//! Tokenizer for `memory.x` expressions.
//!
//! Splits an expression string into alternating [`Token::Atom`] and
//! [`Token::Op`] values. Parenthesised sub-expressions are kept intact as a
//! single atom so the expression evaluator can recurse into them.

use std::error::Error;

/// A single unit produced by the tokenizer.
#[derive(Debug)]
pub enum Token {
    /// A value atom: a numeric literal, a `K`/`M`-suffixed literal,
    /// an `ORIGIN(NAME)` / `LENGTH(NAME)` call, or a `(…)` sub-expression.
    Atom(String),
    /// A binary operator (`+` or `-`).
    Op(char),
}

impl Token {
    pub fn as_str(&self) -> &str {
        match self {
            Token::Atom(s) => s.as_str(),
            Token::Op('+') => "+",
            Token::Op('-') => "-",
            Token::Op(_) => "",
        }
    }
}

/// Split `expr` into alternating atom / operator tokens.
///
/// Operators inside parentheses are not treated as token boundaries, so
/// `ORIGIN(RAM) + LENGTH(RAM)` produces three tokens:
/// `Atom("ORIGIN(RAM)")`, `Op('+')`, `Atom("LENGTH(RAM)")`.
pub fn tokenise(expr: &str) -> Result<Vec<Token>, Box<dyn Error>> {
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
