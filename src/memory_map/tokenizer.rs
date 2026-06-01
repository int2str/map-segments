//! Tokenizer for `memory.x` linker scripts and expressions.
//!
//! Splits source text into structural tokens. Horizontal whitespace is ignored,
//! while newlines are preserved as [`Token::Newline`].

use std::error::Error;
use std::iter::Peekable;

/// A single unit produced by the tokenizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// A parsed numeric literal, including optional `K` / `M` suffixes.
    Number(u64),
    /// An identifier, such as `ORIGIN`, `LENGTH`, or a region name.
    Identifier(String),
    /// A binary `+` operator.
    Plus,
    /// A binary `-` operator.
    Minus,
    /// A `(` delimiter.
    LeftParen,
    /// A `)` delimiter.
    RightParen,
    /// A `{` delimiter.
    LeftBrace,
    /// A `}` delimiter.
    RightBrace,
    /// A `:` delimiter.
    Colon,
    /// A `,` delimiter.
    Comma,
    /// A `=` delimiter.
    Equals,
    /// A line boundary.
    Newline,
}

/// Split `source` into structural tokens.
pub fn tokenize(source: &str) -> Result<Vec<Token>, Box<dyn Error>> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '\n' => {
                chars.next();
                tokens.push(Token::Newline);
            }
            '\r' => {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                tokens.push(Token::Newline);
            }
            ch if ch.is_whitespace() => {
                chars.next();
            }
            '/' => {
                chars.next();
                match chars.peek() {
                    Some('/') => skip_line_comment(&mut chars),
                    Some('*') => skip_block_comment(&mut chars, &mut tokens)?,
                    _ => {
                        return Err(
                            format!("Unexpected character '/' in source: '{source}'").into()
                        );
                    }
                }
            }
            ch if is_identifier_start(ch) => {
                tokens.push(Token::Identifier(read_identifier(&mut chars)))
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LeftParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RightParen);
            }
            '{' => {
                chars.next();
                tokens.push(Token::LeftBrace);
            }
            '}' => {
                chars.next();
                tokens.push(Token::RightBrace);
            }
            ':' => {
                chars.next();
                tokens.push(Token::Colon);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '=' => {
                chars.next();
                tokens.push(Token::Equals);
            }
            '0'..='9' => tokens.push(Token::Number(read_number(&mut chars)?)),
            _ => return Err(format!("Unexpected character '{ch}' in source: '{source}'").into()),
        }
    }

    Ok(tokens)
}

fn read_number<I>(chars: &mut Peekable<I>) -> Result<u64, Box<dyn Error>>
where
    I: Iterator<Item = char>,
{
    let mut raw = String::new();
    raw.push(chars.next().expect("number starts with a digit"));

    let value = if raw == "0" && matches!(chars.peek(), Some('x' | 'X')) {
        chars.next();
        while matches!(chars.peek(), Some(ch) if ch.is_ascii_hexdigit()) {
            raw.push(chars.next().expect("peeked a hex digit"));
        }
        if raw.len() == 1 {
            return Err("Invalid numeric literal: '0x'".into());
        }
        u64::from_str_radix(&raw[1..], 16)?
    } else {
        while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
            raw.push(chars.next().expect("peeked a digit"));
        }
        raw.parse::<u64>()?
    };

    apply_suffix(value, chars)
}

fn apply_suffix<I>(value: u64, chars: &mut Peekable<I>) -> Result<u64, Box<dyn Error>>
where
    I: Iterator<Item = char>,
{
    while matches!(chars.peek(), Some(' ' | '\t')) {
        chars.next();
    }

    match chars.peek() {
        Some('k' | 'K') => {
            chars.next();
            Ok(value.saturating_mul(1_024))
        }
        Some('m' | 'M') => {
            chars.next();
            Ok(value.saturating_mul(1_024).saturating_mul(1_024))
        }
        _ => Ok(value),
    }
}

fn skip_line_comment<I>(chars: &mut Peekable<I>)
where
    I: Iterator<Item = char>,
{
    chars.next();
    while let Some(&ch) = chars.peek() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        chars.next();
    }
}

fn skip_block_comment<I>(
    chars: &mut Peekable<I>,
    tokens: &mut Vec<Token>,
) -> Result<(), Box<dyn Error>>
where
    I: Iterator<Item = char>,
{
    chars.next();

    let mut previous_was_star = false;
    while let Some(ch) = chars.next() {
        match ch {
            '/' if previous_was_star => return Ok(()),
            '\n' => {
                tokens.push(Token::Newline);
                previous_was_star = false;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                tokens.push(Token::Newline);
                previous_was_star = false;
            }
            '*' => previous_was_star = true,
            _ => previous_was_star = false,
        }
    }

    Err("Unterminated block comment".into())
}

fn read_identifier<I>(chars: &mut Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut identifier = String::new();
    while matches!(chars.peek(), Some(&ch) if is_identifier_continue(ch)) {
        identifier.push(chars.next().expect("peeked an identifier character"));
    }
    identifier
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::{Token, tokenize};

    // -- Successful cases -----------------------------------------------------

    #[test]
    fn decimal_zero() {
        assert_eq!(tokenize("0").unwrap(), vec![Token::Number(0)]);
    }

    #[test]
    fn decimal_plain() {
        assert_eq!(tokenize("1234").unwrap(), vec![Token::Number(1234)]);
    }

    #[test]
    fn decimal_large() {
        assert_eq!(
            tokenize("4294967295").unwrap(),
            vec![Token::Number(4_294_967_295)]
        );
    }

    #[test]
    fn hex_lowercase_prefix() {
        assert_eq!(tokenize("0x1000").unwrap(), vec![Token::Number(0x1000)]);
    }

    #[test]
    fn hex_uppercase_prefix() {
        assert_eq!(tokenize("0X1000").unwrap(), vec![Token::Number(0x1000)]);
    }

    #[test]
    fn hex_mixed_case_digits() {
        assert_eq!(
            tokenize("0xDeAdBeEf").unwrap(),
            vec![Token::Number(0xDEAD_BEEF)]
        );
    }

    #[test]
    fn hex_zero() {
        assert_eq!(tokenize("0x0").unwrap(), vec![Token::Number(0)]);
    }

    #[test]
    fn hex_large() {
        assert_eq!(
            tokenize("0x20000000").unwrap(),
            vec![Token::Number(0x2000_0000)]
        );
    }

    #[test]
    fn suffix_k_lowercase() {
        assert_eq!(tokenize("16k").unwrap(), vec![Token::Number(16 * 1_024)]);
    }

    #[test]
    fn suffix_k_uppercase() {
        assert_eq!(tokenize("16K").unwrap(), vec![Token::Number(16 * 1_024)]);
    }

    #[test]
    fn suffix_m_lowercase() {
        assert_eq!(
            tokenize("2m").unwrap(),
            vec![Token::Number(2 * 1_024 * 1_024)]
        );
    }

    #[test]
    fn suffix_m_uppercase() {
        assert_eq!(
            tokenize("2M").unwrap(),
            vec![Token::Number(2 * 1_024 * 1_024)]
        );
    }

    #[test]
    fn suffix_k_zero() {
        assert_eq!(tokenize("0K").unwrap(), vec![Token::Number(0)]);
    }

    #[test]
    fn whitespace_ignored() {
        assert_eq!(
            tokenize(" ORIGIN ( RAM ) + 16 K ").unwrap(),
            vec![
                Token::Identifier("ORIGIN".to_string()),
                Token::LeftParen,
                Token::Identifier("RAM".to_string()),
                Token::RightParen,
                Token::Plus,
                Token::Number(16 * 1_024),
            ]
        );
    }

    #[test]
    fn expression_tokens() {
        assert_eq!(
            tokenize("ORIGIN(RAM)+LENGTH(RAM)").unwrap(),
            vec![
                Token::Identifier("ORIGIN".to_string()),
                Token::LeftParen,
                Token::Identifier("RAM".to_string()),
                Token::RightParen,
                Token::Plus,
                Token::Identifier("LENGTH".to_string()),
                Token::LeftParen,
                Token::Identifier("RAM".to_string()),
                Token::RightParen,
            ]
        );
    }

    // -- Failure cases --------------------------------------------------------

    #[test]
    fn suffix_without_number_is_identifier() {
        assert_eq!(
            tokenize("K").unwrap(),
            vec![Token::Identifier("K".to_string())]
        );
    }

    #[test]
    fn hex_invalid_digit() {
        assert!(tokenize("0xGHIJ").is_err());
    }

    #[test]
    fn float_rejected() {
        assert!(tokenize("1.5").is_err());
    }

    #[test]
    fn hex_prefix_only() {
        assert!(tokenize("0x").is_err());
    }
}
