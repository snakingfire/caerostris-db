//! The lexical token set for openCypher.
//!
//! [`Token`] is the output of the [`lexer`](crate::cypher::lexer): a flat stream
//! of keywords, identifiers, literals, operators, and punctuation, each tagged
//! with its source [`Location`]. Keywords are kept as a single [`Token::Keyword`]
//! variant carrying a [`Keyword`] enum (rather than one variant per keyword) so
//! the parser matches on a small, exhaustive set and the lexer stays compact.

use super::error::Location;

/// An openCypher reserved word. Cypher keywords are **case-insensitive**
/// (`match`, `MATCH`, and `MaTcH` are the same keyword); the lexer upper-cases
/// before lookup and stores the canonical [`Keyword`] here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Match,
    Optional,
    Where,
    Return,
    With,
    Unwind,
    Order,
    By,
    Skip,
    Limit,
    As,
    Distinct,
    And,
    Or,
    Xor,
    Not,
    In,
    Is,
    Null,
    True,
    False,
    Asc,
    Ascending,
    Desc,
    Descending,
    Starts,
    Ends,
    Contains,
    Count,
}

impl Keyword {
    /// Map an already-upper-cased word to its [`Keyword`], or `None` if it is an
    /// ordinary identifier.
    #[must_use]
    pub(crate) fn from_upper(word: &str) -> Option<Keyword> {
        Some(match word {
            "MATCH" => Keyword::Match,
            "OPTIONAL" => Keyword::Optional,
            "WHERE" => Keyword::Where,
            "RETURN" => Keyword::Return,
            "WITH" => Keyword::With,
            "UNWIND" => Keyword::Unwind,
            "ORDER" => Keyword::Order,
            "BY" => Keyword::By,
            "SKIP" => Keyword::Skip,
            "LIMIT" => Keyword::Limit,
            "AS" => Keyword::As,
            "DISTINCT" => Keyword::Distinct,
            "AND" => Keyword::And,
            "OR" => Keyword::Or,
            "XOR" => Keyword::Xor,
            "NOT" => Keyword::Not,
            "IN" => Keyword::In,
            "IS" => Keyword::Is,
            "NULL" => Keyword::Null,
            "TRUE" => Keyword::True,
            "FALSE" => Keyword::False,
            "ASC" => Keyword::Asc,
            "ASCENDING" => Keyword::Ascending,
            "DESC" => Keyword::Desc,
            "DESCENDING" => Keyword::Descending,
            "STARTS" => Keyword::Starts,
            "ENDS" => Keyword::Ends,
            "CONTAINS" => Keyword::Contains,
            "COUNT" => Keyword::Count,
            _ => return None,
        })
    }
}

/// A lexical token kind. Literals carry their already-decoded value (string
/// escapes processed, integers/floats parsed) so the parser builds AST literals
/// directly without re-lexing.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// A reserved word (case-insensitive in the source).
    Keyword(Keyword),
    /// An identifier: a label, relationship type, property key, variable, or
    /// function name. Stored verbatim (case-sensitive).
    Identifier(String),
    /// A parameter reference `$name` or `$0`; the inner string is the name
    /// (without the `$`).
    Parameter(String),
    /// An integer literal (decimal or `0x`/`0o` radix), decoded to `i64`.
    Integer(i64),
    /// A floating-point literal, decoded to `f64`.
    Float(f64),
    /// A string literal with escapes resolved (the surrounding quotes removed).
    String(String),

    // --- punctuation ---
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `|`
    Pipe,
    /// `;`
    Semicolon,
    /// `$` (a bare dollar not followed by a name; rare, kept for completeness).
    Dollar,
    /// `*`
    Star,

    // --- relationship arrows / dashes ---
    /// `-`
    Dash,
    /// `<` (left of an arrow, or the less-than operator).
    Lt,
    /// `>` (right of an arrow, or the greater-than operator).
    Gt,

    // --- operators ---
    /// `+`
    Plus,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `^`
    Caret,
    /// `=`
    Eq,
    /// `<>` (not-equal).
    Neq,
    /// `<=`
    Lte,
    /// `>=`
    Gte,
    /// `..` (range, used by var-length patterns and list slices).
    DotDot,

    /// End of input. Always the final token, so the parser can match on it
    /// instead of bounds-checking the stream.
    Eof,
}

/// A token plus the source [`Location`] at which it begins.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token's kind and decoded value.
    pub kind: TokenKind,
    /// Where the token begins in the source.
    pub location: Location,
}

impl Token {
    pub(crate) fn new(kind: TokenKind, location: Location) -> Self {
        Token { kind, location }
    }
}
