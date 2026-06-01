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

use super::Region;
use super::tokenizer::Token;
#[cfg(test)]
use super::tokenizer::tokenize;

enum Operator {
    Plus,
    Minus,
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
    resolved: &'a [Region],
}

/// Evaluate a `memory.x` expression and return its value.
///
/// `resolved` is the list of regions already parsed; it is used to look up
/// `ORIGIN(NAME)` and `LENGTH(NAME)` references.
#[cfg(test)]
fn evaluate_expression(expression: &str, resolved: &[Region]) -> Result<u64, Box<dyn Error>> {
    let tokens = tokenize(expression)?;
    evaluate_tokens(&tokens, resolved)
}

pub(super) fn evaluate_tokens(
    tokens: &[Token],
    resolved: &[Region],
) -> Result<u64, Box<dyn Error>> {
    if tokens.is_empty() {
        return Err("Empty expression".into());
    }

    let mut parser = Parser {
        tokens,
        index: 0,
        resolved,
    };
    let value = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(value)
}

impl<'a> Parser<'a> {
    fn parse_expression(&mut self) -> Result<u64, Box<dyn Error>> {
        let mut result = self.parse_value()?;

        while let Some(operator) = self.next_operator() {
            let operand = self.parse_value()?;
            match operator {
                Operator::Plus => result = result.wrapping_add(operand),
                Operator::Minus => result = result.wrapping_sub(operand),
            }
        }

        Ok(result)
    }

    fn parse_value(&mut self) -> Result<u64, Box<dyn Error>> {
        match self.next() {
            Some(Token::Number(value)) => Ok(value),
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("ORIGIN") => {
                self.parse_region_reference(|region| region.start)
            }
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("LENGTH") => {
                self.parse_region_reference(|region| region.length)
            }
            Some(Token::Identifier(name)) => Err(format!("Unexpected identifier '{name}'").into()),
            Some(Token::LeftParen) => {
                let value = self.parse_expression()?;
                self.expect_right_paren()?;
                Ok(value)
            }
            Some(token) => Err(format!("Expected value, found {token:?}").into()),
            None => Err("Expected value, found end of expression".into()),
        }
    }

    fn parse_region_reference(&mut self, field: fn(&Region) -> u64) -> Result<u64, Box<dyn Error>> {
        self.expect_left_paren()?;
        let name = match self.next() {
            Some(Token::Identifier(name)) => name,
            Some(token) => return Err(format!("Expected region name, found {token:?}").into()),
            None => return Err("Expected region name, found end of expression".into()),
        };
        self.expect_right_paren()?;
        lookup(&name, self.resolved, field)
    }

    fn next_operator(&mut self) -> Option<Operator> {
        match self.peek() {
            Some(Token::Plus) => {
                self.index += 1;
                Some(Operator::Plus)
            }
            Some(Token::Minus) => {
                self.index += 1;
                Some(Operator::Minus)
            }
            _ => None,
        }
    }

    fn expect_left_paren(&mut self) -> Result<(), Box<dyn Error>> {
        match self.next() {
            Some(Token::LeftParen) => Ok(()),
            Some(token) => Err(format!("Expected '(', found {token:?}").into()),
            None => Err("Expected '(', found end of expression".into()),
        }
    }

    fn expect_right_paren(&mut self) -> Result<(), Box<dyn Error>> {
        match self.next() {
            Some(Token::RightParen) => Ok(()),
            Some(token) => Err(format!("Expected ')', found {token:?}").into()),
            None => Err("Expected ')', found end of expression".into()),
        }
    }

    fn expect_end(&self) -> Result<(), Box<dyn Error>> {
        match self.peek() {
            None => Ok(()),
            Some(token) => Err(format!("Unexpected token {token:?}").into()),
        }
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        self.index += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }
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

#[cfg(test)]
mod tests {
    use super::evaluate_expression;
    use crate::memory_map::Region;

    fn ram_region() -> Vec<Region> {
        vec![Region {
            name: "RAM".to_string(),
            start: 0x2000_0000,
            length: 64 * 1_024,
        }]
    }

    #[test]
    fn evaluates_number() {
        assert_eq!(evaluate_expression("16K", &[]).unwrap(), 16 * 1_024);
    }

    #[test]
    fn evaluates_left_to_right_expression() {
        assert_eq!(
            evaluate_expression("16K + 4 - 2", &[]).unwrap(),
            16 * 1_024 + 2
        );
    }

    #[test]
    fn evaluates_parenthesised_expression() {
        assert_eq!(
            evaluate_expression("(16K + 4) - 2", &[]).unwrap(),
            16 * 1_024 + 2
        );
    }

    #[test]
    fn evaluates_origin_reference() {
        assert_eq!(
            evaluate_expression("ORIGIN(RAM)", &ram_region()).unwrap(),
            0x2000_0000
        );
    }

    #[test]
    fn evaluates_length_reference() {
        assert_eq!(
            evaluate_expression("LENGTH(RAM)", &ram_region()).unwrap(),
            64 * 1_024
        );
    }

    #[test]
    fn ignores_whitespace() {
        assert_eq!(
            evaluate_expression(" ORIGIN ( RAM ) + 16 K ", &ram_region()).unwrap(),
            0x2000_0000 + 16 * 1_024
        );
    }

    #[test]
    fn rejects_unknown_region() {
        assert!(evaluate_expression("ORIGIN(FLASH)", &ram_region()).is_err());
    }

    #[test]
    fn rejects_trailing_tokens() {
        assert!(evaluate_expression("16K 4", &[]).is_err());
    }
}
