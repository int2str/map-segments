//! Parser for `memory.x` linker scripts.
//!
//! Extracts the `MEMORY { ... }` block from a linker script, then parses each
//! region declaration by evaluating its `ORIGIN` and `LENGTH` expressions.

use std::error::Error;
use std::iter::Peekable;
use std::vec::IntoIter;

use super::Region;
use super::expression::evaluate_tokens;
use super::tokenizer::{Token, tokenize};

struct Parser {
    tokens: Peekable<IntoIter<Token>>,
    regions: Vec<Region>,
}

enum AssignmentKind {
    Origin,
    Length,
}

/// Parse a `memory.x` source string into a list of [`Region`]s.
pub fn parse(source: &str) -> Result<Vec<Region>, Box<dyn Error>> {
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        tokens: tokens.into_iter().peekable(),
        regions: Vec::new(),
    };
    parser.parse_memory_block()?;
    Ok(parser.regions)
}

impl Parser {
    fn parse_memory_block(&mut self) -> Result<(), Box<dyn Error>> {
        self.skip_until_memory()?;
        self.skip_newlines();
        self.expect_next(Token::LeftBrace, "{")?;

        loop {
            self.skip_newlines();
            match self.tokens.peek() {
                Some(Token::RightBrace) => {
                    self.tokens.next();
                    return Ok(());
                }
                Some(Token::Identifier(_)) => self.parse_region()?,
                Some(token) => return Err(format!("Expected region name, found {token:?}").into()),
                None => return Err("No closing '}' found for MEMORY block".into()),
            }
        }
    }

    fn skip_until_memory(&mut self) -> Result<(), Box<dyn Error>> {
        for token in self.tokens.by_ref() {
            if let Token::Identifier(name) = token
                && name.eq_ignore_ascii_case("MEMORY")
            {
                return Ok(());
            }
        }

        Err("No MEMORY block found in linker script".into())
    }

    fn parse_region(&mut self) -> Result<(), Box<dyn Error>> {
        let name = match self.tokens.next() {
            Some(Token::Identifier(name)) => name,
            Some(token) => return Err(format!("Expected region name, found {token:?}").into()),
            None => return Err("Expected region name, found end of input".into()),
        };

        if self.tokens.peek() == Some(&Token::LeftParen) {
            self.skip_parenthesized_attributes()?;
        }

        self.expect_next(Token::Colon, ":")?;
        let (first_kind, first_value) = self.parse_assignment(Token::Comma)?;
        self.expect_next(Token::Comma, ",")?;
        let (second_kind, second_value) = self.parse_assignment_until_line_end()?;

        let (start, length) = match (first_kind, second_kind) {
            (AssignmentKind::Origin, AssignmentKind::Length) => (first_value, second_value),
            (AssignmentKind::Length, AssignmentKind::Origin) => (second_value, first_value),
            _ => return Err(format!("Region '{name}' must define ORIGIN and LENGTH").into()),
        };

        self.regions.push(Region {
            name,
            start,
            length,
        });
        self.skip_newlines();
        Ok(())
    }

    fn skip_parenthesized_attributes(&mut self) -> Result<(), Box<dyn Error>> {
        self.expect_next(Token::LeftParen, "(")?;
        let mut depth = 1usize;
        for token in self.tokens.by_ref() {
            match token {
                Token::LeftParen => depth += 1,
                Token::RightParen => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        Err("Unclosed region attributes".into())
    }

    fn parse_assignment(
        &mut self,
        terminator: Token,
    ) -> Result<(AssignmentKind, u64), Box<dyn Error>> {
        let kind = self.parse_assignment_kind()?;
        self.expect_next(Token::Equals, "=")?;
        let expression = self.collect_expression_until(|token| token == &terminator);
        let value = evaluate_tokens(&expression, &self.regions)?;
        Ok((kind, value))
    }

    fn parse_assignment_until_line_end(&mut self) -> Result<(AssignmentKind, u64), Box<dyn Error>> {
        let kind = self.parse_assignment_kind()?;
        self.expect_next(Token::Equals, "=")?;
        let expression = self
            .collect_expression_until(|token| matches!(token, Token::Newline | Token::RightBrace));
        let value = evaluate_tokens(&expression, &self.regions)?;
        Ok((kind, value))
    }

    fn parse_assignment_kind(&mut self) -> Result<AssignmentKind, Box<dyn Error>> {
        match self.tokens.next() {
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("ORIGIN") => {
                Ok(AssignmentKind::Origin)
            }
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("org") => {
                Ok(AssignmentKind::Origin)
            }
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("LENGTH") => {
                Ok(AssignmentKind::Length)
            }
            Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("len") => {
                Ok(AssignmentKind::Length)
            }
            Some(token) => {
                Err(format!("Expected ORIGIN or LENGTH assignment, found {token:?}").into())
            }
            None => Err("Expected ORIGIN or LENGTH assignment, found end of input".into()),
        }
    }

    fn collect_expression_until(&mut self, terminates: impl Fn(&Token) -> bool) -> Vec<Token> {
        let mut expression = Vec::new();

        while self.tokens.peek().is_some_and(|token| !terminates(token)) {
            if let Some(token) = self.tokens.next()
                && !matches!(token, Token::Newline)
            {
                expression.push(token);
            }
        }

        expression
    }

    fn skip_newlines(&mut self) {
        while self.tokens.peek() == Some(&Token::Newline) {
            self.tokens.next();
        }
    }

    fn expect_next(&mut self, expected: Token, label: &str) -> Result<(), Box<dyn Error>> {
        match self.tokens.next() {
            Some(token) if token == expected => Ok(()),
            Some(token) => Err(format!("Expected '{label}', found {token:?}").into()),
            None => Err(format!("Expected '{label}', found end of input").into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_memory_block() {
        let regions = parse(
            r#"
            MEMORY {
                FLASH : ORIGIN = 0x08000000, LENGTH = 256K
                RAM : ORIGIN = 0x20000000, LENGTH = 64K
            }
            "#,
        )
        .unwrap();

        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].start, 0x0800_0000);
        assert_eq!(regions[0].length, 256 * 1_024);
        assert_eq!(regions[1].name, "RAM");
        assert_eq!(regions[1].start, 0x2000_0000);
        assert_eq!(regions[1].length, 64 * 1_024);
    }

    #[test]
    fn parses_memory_block_with_brace_on_next_line() {
        let regions = parse(
            r#"
            MEMORY
            {
                FLASH : ORIGIN = 0x08000000, LENGTH = 256K
            }
            "#,
        )
        .unwrap();

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].start, 0x0800_0000);
        assert_eq!(regions[0].length, 256 * 1_024);
    }

    #[test]
    fn parses_region_attributes_and_lowercase_assignment_names() {
        let regions = parse(
            r#"
            MEMORY {
                FLASH (rx) : org = 0x08000000, len = 256K
            }
            "#,
        )
        .unwrap();

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].start, 0x0800_0000);
        assert_eq!(regions[0].length, 256 * 1_024);
    }

    #[test]
    fn parses_comments() {
        let regions = parse(
            r#"
            // Leading comment
            MEMORY {
                /* block comment */
                FLASH : ORIGIN = 0x08000000, LENGTH = 256K // trailing comment
                /* multiline
                   block comment */
                RAM : ORIGIN = ORIGIN(FLASH) + LENGTH(FLASH), LENGTH = 64K
            }
            "#,
        )
        .unwrap();

        assert_eq!(regions.len(), 2);
        assert_eq!(regions[1].name, "RAM");
        assert_eq!(regions[1].start, 0x0800_0000 + 256 * 1_024);
        assert_eq!(regions[1].length, 64 * 1_024);
    }

    #[test]
    fn parses_reversed_assignment_order() {
        let regions = parse(
            r#"
            MEMORY {
                RAM : LENGTH = 64K, ORIGIN = 0x20000000
            }
            "#,
        )
        .unwrap();

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start, 0x2000_0000);
        assert_eq!(regions[0].length, 64 * 1_024);
    }

    #[test]
    fn rejects_missing_memory_block() {
        assert!(parse("FLASH : ORIGIN = 0, LENGTH = 1K").is_err());
    }

    #[test]
    fn rejects_unterminated_block_comment() {
        assert!(parse("MEMORY { /* nope").is_err());
    }
}
