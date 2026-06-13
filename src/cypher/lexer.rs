//! The openCypher lexer: source text → a [`Vec<Token>`] ending in
//! [`TokenKind::Eof`].
//!
//! The lexer is a hand-written single-pass scanner over the source `char`s. It
//! tracks a 1-based line/column [`Location`] for every token so the parser can
//! report errors at the right place. Cypher keywords are case-insensitive, so a
//! word is upper-cased and looked up in [`Keyword::from_upper`] before falling
//! back to an identifier.
//!
//! What it recognises:
//! - keywords and identifiers (`[A-Za-z_][A-Za-z0-9_]*`, plus backtick-quoted
//!   identifiers `` `weird name` ``);
//! - parameters `$name` and `$0`;
//! - integer literals (decimal, `0x` hex, `0o` octal) and float literals
//!   (`1.5`, `1e10`, `.5`, `6.022e23`);
//! - string literals in single or double quotes, with the openCypher escape set;
//! - the operator and punctuation set in [`TokenKind`];
//! - `//` line comments and `/* ... */` block comments (skipped).

use super::error::{CypherError, CypherResult, Location};
use super::token::{Keyword, Token, TokenKind};

/// Tokenise `src` into a stream of [`Token`]s terminated by [`TokenKind::Eof`].
///
/// # Errors
/// Returns a [`CypherError`] of kind `Lex` on an illegal character, an
/// unterminated string or block comment, or a malformed number.
pub fn tokenize(src: &str) -> CypherResult<Vec<Token>> {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    src: &'a str,
    /// The remaining input as a peekable char-indexed cursor.
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            src,
            chars: src.char_indices().peekable(),
            line: 1,
            column: 1,
        }
    }

    /// The location of the next char to be consumed (byte offset from the
    /// peeked index, or end-of-input).
    fn here(&mut self) -> Location {
        let offset = self.chars.peek().map_or(self.src.len(), |&(i, _)| i);
        Location {
            line: self.line,
            column: self.column,
            offset,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    /// Look one char past the cursor without consuming.
    fn peek2(&self) -> Option<char> {
        self.chars.clone().nth(1).map(|(_, c)| c)
    }

    /// Consume and return the next char, advancing line/column tracking.
    fn bump(&mut self) -> Option<char> {
        let (_, c) = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn run(mut self) -> CypherResult<Vec<Token>> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia()?;
            let loc = self.here();
            let Some(c) = self.peek() else {
                out.push(Token::new(TokenKind::Eof, loc));
                return Ok(out);
            };
            let kind = self.scan_token(c, loc)?;
            out.push(Token::new(kind, loc));
        }
    }

    /// Skip whitespace, `//` line comments, and `/* ... */` block comments.
    fn skip_trivia(&mut self) -> CypherResult<()> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek2() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek2() == Some('*') => {
                    let start = self.here();
                    self.bump(); // /
                    self.bump(); // *
                    loop {
                        match self.bump() {
                            Some('*') if self.peek() == Some('/') => {
                                self.bump();
                                break;
                            }
                            Some(_) => {}
                            None => {
                                return Err(CypherError::lex(start, "unterminated block comment"));
                            }
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn scan_token(&mut self, c: char, loc: Location) -> CypherResult<TokenKind> {
        match c {
            '(' => self.single(TokenKind::LParen),
            ')' => self.single(TokenKind::RParen),
            '[' => self.single(TokenKind::LBracket),
            ']' => self.single(TokenKind::RBracket),
            '{' => self.single(TokenKind::LBrace),
            '}' => self.single(TokenKind::RBrace),
            ',' => self.single(TokenKind::Comma),
            ':' => self.single(TokenKind::Colon),
            ';' => self.single(TokenKind::Semicolon),
            '|' => self.single(TokenKind::Pipe),
            '+' => self.single(TokenKind::Plus),
            '*' => self.single(TokenKind::Star),
            '/' => self.single(TokenKind::Slash),
            '%' => self.single(TokenKind::Percent),
            '^' => self.single(TokenKind::Caret),
            '=' => self.single(TokenKind::Eq),
            '-' => self.single(TokenKind::Dash),
            '.' => self.scan_dot(),
            '<' => self.scan_lt(),
            '>' => self.scan_gt(),
            '$' => self.scan_parameter(),
            '\'' | '"' => self.scan_string(c, loc),
            '`' => self.scan_quoted_identifier(loc),
            '0'..='9' => self.scan_number(loc),
            c if is_ident_start(c) => Ok(self.scan_word()),
            other => Err(CypherError::lex(
                loc,
                format!("unexpected character '{other}'"),
            )),
        }
    }

    /// Consume one char and yield `kind`.
    fn single(&mut self, kind: TokenKind) -> CypherResult<TokenKind> {
        self.bump();
        Ok(kind)
    }

    /// `.` may begin `..` (range) or `.5` (float) or be a lone member-access dot.
    fn scan_dot(&mut self) -> CypherResult<TokenKind> {
        if self.peek2() == Some('.') {
            self.bump();
            self.bump();
            return Ok(TokenKind::DotDot);
        }
        if matches!(self.peek2(), Some('0'..='9')) {
            // A leading-dot float like `.5`; reuse the number scanner.
            let loc = self.here();
            return self.scan_number(loc);
        }
        self.single(TokenKind::Dot)
    }

    /// `<` may begin `<=` or `<>` or be a lone `<`.
    fn scan_lt(&mut self) -> CypherResult<TokenKind> {
        self.bump();
        match self.peek() {
            Some('=') => {
                self.bump();
                Ok(TokenKind::Lte)
            }
            Some('>') => {
                self.bump();
                Ok(TokenKind::Neq)
            }
            _ => Ok(TokenKind::Lt),
        }
    }

    /// `>` may begin `>=` or be a lone `>`.
    fn scan_gt(&mut self) -> CypherResult<TokenKind> {
        self.bump();
        if self.peek() == Some('=') {
            self.bump();
            Ok(TokenKind::Gte)
        } else {
            Ok(TokenKind::Gt)
        }
    }

    /// `$name` / `$0`, or a bare `$`.
    fn scan_parameter(&mut self) -> CypherResult<TokenKind> {
        self.bump(); // $
        let mut name = String::new();
        // A parameter name is an identifier or a run of digits (`$0`).
        match self.peek() {
            Some(c) if is_ident_start(c) => {
                while let Some(c) = self.peek() {
                    if is_ident_continue(c) {
                        name.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            Some(c) if c.is_ascii_digit() => {
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        name.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            _ => return Ok(TokenKind::Dollar),
        }
        Ok(TokenKind::Parameter(name))
    }

    /// A keyword or identifier word. `c` (the first char) is left in the stream.
    fn scan_word(&mut self) -> TokenKind {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                word.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if let Some(kw) = Keyword::from_upper(&word.to_ascii_uppercase()) {
            TokenKind::Keyword(kw)
        } else {
            TokenKind::Identifier(word)
        }
    }

    /// A backtick-quoted identifier `` `any chars` ``; doubled backticks escape a
    /// literal backtick.
    fn scan_quoted_identifier(&mut self, loc: Location) -> CypherResult<TokenKind> {
        self.bump(); // opening backtick
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('`') => {
                    if self.peek() == Some('`') {
                        self.bump();
                        s.push('`');
                    } else {
                        return Ok(TokenKind::Identifier(s));
                    }
                }
                Some(c) => s.push(c),
                None => return Err(CypherError::lex(loc, "unterminated quoted identifier")),
            }
        }
    }

    /// A string literal in `'` or `"` quotes with openCypher escapes.
    fn scan_string(&mut self, quote: char, loc: Location) -> CypherResult<TokenKind> {
        self.bump(); // opening quote
        let mut s = String::new();
        loop {
            match self.bump() {
                Some(c) if c == quote => return Ok(TokenKind::String(s)),
                Some('\\') => {
                    let esc_loc = self.here();
                    let Some(e) = self.bump() else {
                        return Err(CypherError::lex(loc, "unterminated string literal"));
                    };
                    s.push(self.decode_escape(e, esc_loc)?);
                }
                Some(c) => s.push(c),
                None => return Err(CypherError::lex(loc, "unterminated string literal")),
            }
        }
    }

    /// Decode the character following a backslash in a string literal.
    fn decode_escape(&mut self, e: char, loc: Location) -> CypherResult<char> {
        Ok(match e {
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            'b' => '\u{0008}',
            'f' => '\u{000C}',
            '0' => '\0',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            '`' => '`',
            'u' => self.decode_unicode_escape(loc)?,
            other => {
                return Err(CypherError::lex(
                    loc,
                    format!("invalid escape sequence '\\{other}'"),
                ));
            }
        })
    }

    /// Decode a `\uXXXX` 4-hex-digit Unicode escape (the `\u` has been consumed).
    fn decode_unicode_escape(&mut self, loc: Location) -> CypherResult<char> {
        let mut code = 0u32;
        for _ in 0..4 {
            let Some(c) = self.bump() else {
                return Err(CypherError::lex(loc, "truncated \\u escape"));
            };
            let Some(d) = c.to_digit(16) else {
                return Err(CypherError::lex(
                    loc,
                    format!("invalid hex digit '{c}' in \\u escape"),
                ));
            };
            code = code * 16 + d;
        }
        char::from_u32(code).ok_or_else(|| {
            CypherError::lex(loc, format!("invalid Unicode code point U+{code:04X}"))
        })
    }

    /// A numeric literal: integer (decimal/hex/octal) or float.
    fn scan_number(&mut self, loc: Location) -> CypherResult<TokenKind> {
        let mut raw = String::new();
        // Radix prefixes: 0x / 0o.
        if self.peek() == Some('0') && matches!(self.peek2(), Some('x' | 'X' | 'o' | 'O')) {
            self.bump(); // 0
            let radix_char = self.bump().expect("peek2 guaranteed a radix char");
            let radix = if radix_char.eq_ignore_ascii_case(&'x') {
                16
            } else {
                8
            };
            let mut digits = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() {
                    digits.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
            let n = i64::from_str_radix(&digits, radix).map_err(|_| {
                CypherError::lex(loc, format!("invalid base-{radix} integer literal"))
            })?;
            return Ok(TokenKind::Integer(n));
        }

        let mut is_float = false;
        // Integer part / leading dot.
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                raw.push(c);
                self.bump();
            } else {
                break;
            }
        }
        // Fractional part.
        if self.peek() == Some('.') && self.peek2() != Some('.') {
            is_float = true;
            raw.push('.');
            self.bump();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    raw.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }
        // Exponent.
        if matches!(self.peek(), Some('e' | 'E')) {
            is_float = true;
            raw.push('e');
            self.bump();
            if matches!(self.peek(), Some('+' | '-')) {
                raw.push(self.bump().expect("just peeked a sign"));
            }
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    raw.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }

        if is_float {
            let f: f64 = raw
                .parse()
                .map_err(|_| CypherError::lex(loc, format!("invalid float literal '{raw}'")))?;
            Ok(TokenKind::Float(f))
        } else {
            let n: i64 = raw.parse().map_err(|_| {
                CypherError::lex(loc, format!("integer literal '{raw}' out of range"))
            })?;
            Ok(TokenKind::Integer(n))
        }
    }
}

/// `true` if `c` may start an unquoted identifier.
fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

/// `true` if `c` may continue an unquoted identifier.
fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cypher::error::CypherErrorKind;

    /// Tokenise and strip the trailing `Eof`, returning only the kinds.
    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut toks = tokenize(src).expect("lexes cleanly");
        assert!(matches!(toks.last().map(|t| &t.kind), Some(TokenKind::Eof)));
        toks.pop();
        toks.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn keywords_are_case_insensitive() {
        for src in ["MATCH", "match", "MaTcH"] {
            assert_eq!(kinds(src), vec![TokenKind::Keyword(Keyword::Match)]);
        }
        assert_eq!(
            kinds("Return Distinct"),
            vec![
                TokenKind::Keyword(Keyword::Return),
                TokenKind::Keyword(Keyword::Distinct)
            ]
        );
    }

    #[test]
    fn identifiers_are_case_sensitive_and_distinct_from_keywords() {
        assert_eq!(
            kinds("nMatch"),
            vec![TokenKind::Identifier("nMatch".into())]
        );
        assert_eq!(
            kinds("Person"),
            vec![TokenKind::Identifier("Person".into())]
        );
        assert_eq!(kinds("_x9"), vec![TokenKind::Identifier("_x9".into())]);
    }

    #[test]
    fn backtick_quoted_identifier_with_escape() {
        assert_eq!(
            kinds("`weird name`"),
            vec![TokenKind::Identifier("weird name".into())]
        );
        // A doubled backtick is an escaped literal backtick.
        assert_eq!(kinds("`a``b`"), vec![TokenKind::Identifier("a`b".into())]);
    }

    #[test]
    fn integer_literals_decimal_hex_octal() {
        assert_eq!(kinds("42"), vec![TokenKind::Integer(42)]);
        assert_eq!(kinds("0"), vec![TokenKind::Integer(0)]);
        assert_eq!(kinds("0x1F"), vec![TokenKind::Integer(0x1F)]);
        assert_eq!(kinds("0o17"), vec![TokenKind::Integer(0o17)]);
    }

    #[test]
    fn float_literals_every_shape() {
        assert_eq!(kinds("1.5"), vec![TokenKind::Float(1.5)]);
        assert_eq!(kinds(".5"), vec![TokenKind::Float(0.5)]);
        assert_eq!(kinds("6.022e23"), vec![TokenKind::Float(6.022e23)]);
        assert_eq!(kinds("1e10"), vec![TokenKind::Float(1e10)]);
        assert_eq!(kinds("2.0E-3"), vec![TokenKind::Float(2.0E-3)]);
    }

    #[test]
    fn string_literals_single_and_double_quote() {
        assert_eq!(kinds("'hello'"), vec![TokenKind::String("hello".into())]);
        assert_eq!(kinds("\"hi\""), vec![TokenKind::String("hi".into())]);
        assert_eq!(kinds("''"), vec![TokenKind::String(String::new())]);
    }

    #[test]
    fn string_escapes_are_decoded() {
        assert_eq!(kinds(r"'a\tb\n'"), vec![TokenKind::String("a\tb\n".into())]);
        assert_eq!(kinds(r"'A'"), vec![TokenKind::String("A".into())]);
        assert_eq!(kinds(r#"'it\'s'"#), vec![TokenKind::String("it's".into())]);
    }

    #[test]
    fn boolean_and_null_literals() {
        assert_eq!(
            kinds("true false null"),
            vec![
                TokenKind::Keyword(Keyword::True),
                TokenKind::Keyword(Keyword::False),
                TokenKind::Keyword(Keyword::Null)
            ]
        );
    }

    #[test]
    fn parameters_named_and_numeric() {
        assert_eq!(kinds("$name"), vec![TokenKind::Parameter("name".into())]);
        assert_eq!(kinds("$0"), vec![TokenKind::Parameter("0".into())]);
        assert_eq!(
            kinds("$friendIndex"),
            vec![TokenKind::Parameter("friendIndex".into())]
        );
    }

    #[test]
    fn operators_and_punctuation() {
        assert_eq!(
            kinds("<= >= <> = < > + - * / % ^"),
            vec![
                TokenKind::Lte,
                TokenKind::Gte,
                TokenKind::Neq,
                TokenKind::Eq,
                TokenKind::Lt,
                TokenKind::Gt,
                TokenKind::Plus,
                TokenKind::Dash,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Caret,
            ]
        );
        assert_eq!(
            kinds("( ) [ ] { } , : . | ; .."),
            vec![
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::Comma,
                TokenKind::Colon,
                TokenKind::Dot,
                TokenKind::Pipe,
                TokenKind::Semicolon,
                TokenKind::DotDot,
            ]
        );
    }

    #[test]
    fn relationship_arrows_lex_into_pieces() {
        // The parser, not the lexer, assembles `<-[ ]->`; the lexer emits parts.
        assert_eq!(
            kinds("-->"),
            vec![TokenKind::Dash, TokenKind::Dash, TokenKind::Gt]
        );
        assert_eq!(
            kinds("<--"),
            vec![TokenKind::Lt, TokenKind::Dash, TokenKind::Dash]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            kinds("RETURN // a line comment\n 1"),
            vec![TokenKind::Keyword(Keyword::Return), TokenKind::Integer(1)]
        );
        assert_eq!(
            kinds("RETURN /* block */ 1"),
            vec![TokenKind::Keyword(Keyword::Return), TokenKind::Integer(1)]
        );
    }

    #[test]
    fn error_position_is_reported_for_illegal_char() {
        // The `@` on line 2, column 8 is illegal.
        let err = tokenize("RETURN 1\nRETURN @x").unwrap_err();
        assert_eq!(err.kind, CypherErrorKind::Lex);
        assert_eq!(err.location.line, 2);
        assert_eq!(err.location.column, 8);
    }

    #[test]
    fn token_locations_track_columns() {
        let toks = tokenize("MATCH (n)").expect("lexes");
        // MATCH at col 1, '(' at col 7, 'n' at col 8, ')' at col 9.
        assert_eq!(toks[0].location.column, 1);
        assert_eq!(toks[1].location.column, 7);
        assert_eq!(toks[2].location.column, 8);
        assert_eq!(toks[3].location.column, 9);
    }

    #[test]
    fn unterminated_string_is_a_structured_error_not_a_panic() {
        let err = tokenize("RETURN 'oops").unwrap_err();
        assert_eq!(err.kind, CypherErrorKind::Lex);
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn unterminated_block_comment_errors() {
        let err = tokenize("RETURN /* never closes").unwrap_err();
        assert_eq!(err.kind, CypherErrorKind::Lex);
        assert!(err.message.contains("block comment"));
    }

    #[test]
    fn invalid_escape_errors() {
        let err = tokenize(r"RETURN '\q'").unwrap_err();
        assert_eq!(err.kind, CypherErrorKind::Lex);
        assert!(err.message.contains("escape"));
    }

    #[test]
    fn empty_input_is_just_eof() {
        let toks = tokenize("   \n  ").expect("lexes");
        assert_eq!(toks.len(), 1);
        assert!(matches!(toks[0].kind, TokenKind::Eof));
    }

    #[test]
    fn multibyte_chars_advance_column_by_one() {
        // 'é' is two bytes but one column; the `@` after it is at column 3.
        let err = tokenize("'é'@").unwrap_err();
        // The string lexes; the stray `@` is the error at column 4 (', é, ', @).
        assert_eq!(err.location.column, 4);
    }
}
