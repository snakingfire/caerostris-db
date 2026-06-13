//! The openCypher recursive-descent parser: a [`Token`] stream → a typed
//! [`Query`] AST.
//!
//! The clause grammar (`MATCH` / `OPTIONAL MATCH`, `UNWIND`, `WITH`, `RETURN`)
//! is parsed top-down; expressions use a precedence-climbing (Pratt) loop that
//! mirrors the openCypher operator precedence:
//!
//! ```text
//! OR  <  XOR  <  AND  <  NOT  <  comparison (= <> < <= > >= IN STARTS/ENDS/CONTAINS, IS NULL)
//!     <  +/-  <  * / %  <  ^  <  unary -  <  postfix (. [] )  <  primary
//! ```
//!
//! Every failure is a structured [`CypherError`] carrying the offending token's
//! [`Location`]; the parser never panics on malformed input.

use crate::model::PropertyValue;

use super::ast::{
    BinaryOp, Clause, Direction, Expr, MatchClause, NodePattern, PathPattern, PatternStep,
    ProjectionClause, ProjectionItem, Query, RelPattern, ReturnBody, SortItem, UnaryOp,
    UnwindClause, VarLength,
};
use super::error::{CypherError, CypherResult, Location};
use super::lexer::tokenize;
use super::token::{Keyword, Token, TokenKind};

/// Parse `src` into a [`Query`] AST.
///
/// # Errors
/// Returns a [`CypherError`] (kind `Lex` or `Parse`) on malformed input, with the
/// source location of the first problem.
pub fn parse(src: &str) -> CypherResult<Query> {
    let tokens = tokenize(src)?;
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_query()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_location(&self) -> Location {
        self.tokens[self.pos].location
    }

    /// Advance and return the consumed token's kind (cloned).
    fn advance(&mut self) -> TokenKind {
        let kind = self.tokens[self.pos].kind.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        kind
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn is_keyword(&self, kw: Keyword) -> bool {
        matches!(self.peek(), TokenKind::Keyword(k) if *k == kw)
    }

    /// Consume the current token if it equals `kind`; otherwise leave it.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, kw: Keyword) -> bool {
        if self.is_keyword(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume the current token, requiring it to equal `kind`.
    fn expect(&mut self, kind: &TokenKind, what: &str) -> CypherResult<()> {
        if self.peek() == kind {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {what}")))
        }
    }

    fn expect_keyword(&mut self, kw: Keyword, what: &str) -> CypherResult<()> {
        if self.eat_keyword(kw) {
            Ok(())
        } else {
            Err(self.error(format!("expected {what}")))
        }
    }

    fn error(&self, message: impl Into<String>) -> CypherError {
        CypherError::parse(self.peek_location(), message)
    }

    // --- clauses ------------------------------------------------------------

    fn parse_query(&mut self) -> CypherResult<Query> {
        let mut clauses = Vec::new();
        // Allow a leading/trailing `;` (single-statement queries).
        self.eat(&TokenKind::Semicolon);
        while !self.at_eof() {
            clauses.push(self.parse_clause()?);
            self.eat(&TokenKind::Semicolon);
        }
        if clauses.is_empty() {
            return Err(self.error("empty query"));
        }
        Ok(Query { clauses })
    }

    fn parse_clause(&mut self) -> CypherResult<Clause> {
        match self.peek() {
            TokenKind::Keyword(Keyword::Match | Keyword::Optional) => {
                Ok(Clause::Match(self.parse_match()?))
            }
            TokenKind::Keyword(Keyword::Unwind) => Ok(Clause::Unwind(self.parse_unwind()?)),
            TokenKind::Keyword(Keyword::With) => {
                self.advance();
                Ok(Clause::With(self.parse_projection(true)?))
            }
            TokenKind::Keyword(Keyword::Return) => {
                self.advance();
                Ok(Clause::Return(self.parse_projection(false)?))
            }
            _ => Err(self.error("expected a clause (MATCH, OPTIONAL MATCH, UNWIND, WITH, RETURN)")),
        }
    }

    fn parse_match(&mut self) -> CypherResult<MatchClause> {
        let optional = self.eat_keyword(Keyword::Optional);
        self.expect_keyword(Keyword::Match, "MATCH")?;
        let mut patterns = vec![self.parse_path_pattern()?];
        while self.eat(&TokenKind::Comma) {
            patterns.push(self.parse_path_pattern()?);
        }
        let where_clause = if self.eat_keyword(Keyword::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(MatchClause {
            optional,
            patterns,
            where_clause,
        })
    }

    fn parse_unwind(&mut self) -> CypherResult<UnwindClause> {
        self.expect_keyword(Keyword::Unwind, "UNWIND")?;
        let expr = self.parse_expr()?;
        self.expect_keyword(Keyword::As, "AS in UNWIND")?;
        let variable = self.parse_identifier("variable name after AS")?;
        Ok(UnwindClause { expr, variable })
    }

    /// Parse a `WITH` (`allow_where = true`) or `RETURN` projection.
    fn parse_projection(&mut self, allow_where: bool) -> CypherResult<ProjectionClause> {
        let distinct = self.eat_keyword(Keyword::Distinct);
        let body = self.parse_return_body()?;

        let mut order_by = Vec::new();
        if self.eat_keyword(Keyword::Order) {
            self.expect_keyword(Keyword::By, "BY after ORDER")?;
            order_by.push(self.parse_sort_item()?);
            while self.eat(&TokenKind::Comma) {
                order_by.push(self.parse_sort_item()?);
            }
        }

        let skip = if self.eat_keyword(Keyword::Skip) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let limit = if self.eat_keyword(Keyword::Limit) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let where_clause = if allow_where && self.eat_keyword(Keyword::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(ProjectionClause {
            distinct,
            body,
            order_by,
            skip,
            limit,
            where_clause,
        })
    }

    fn parse_return_body(&mut self) -> CypherResult<ReturnBody> {
        if self.eat(&TokenKind::Star) {
            // `RETURN *` optionally followed by `, item, item`.
            let mut extra = Vec::new();
            while self.eat(&TokenKind::Comma) {
                extra.push(self.parse_projection_item()?);
            }
            return Ok(ReturnBody::All { extra });
        }
        let mut items = vec![self.parse_projection_item()?];
        while self.eat(&TokenKind::Comma) {
            items.push(self.parse_projection_item()?);
        }
        Ok(ReturnBody::Items(items))
    }

    fn parse_projection_item(&mut self) -> CypherResult<ProjectionItem> {
        let expr = self.parse_expr()?;
        let alias = if self.eat_keyword(Keyword::As) {
            Some(self.parse_identifier("alias after AS")?)
        } else {
            None
        };
        Ok(ProjectionItem { expr, alias })
    }

    fn parse_sort_item(&mut self) -> CypherResult<SortItem> {
        let expr = self.parse_expr()?;
        let descending = if self.eat_keyword(Keyword::Desc) || self.eat_keyword(Keyword::Descending)
        {
            true
        } else {
            // ASC / ASCENDING are the default; consume if present.
            let _ = self.eat_keyword(Keyword::Asc) || self.eat_keyword(Keyword::Ascending);
            false
        };
        Ok(SortItem { expr, descending })
    }

    // --- patterns -----------------------------------------------------------

    fn parse_path_pattern(&mut self) -> CypherResult<PathPattern> {
        // Optional `p = ` path-variable binding. Only a bare identifier followed
        // by `=` is a path binding; a lookahead avoids mis-parsing `(a) = ...`.
        let path_variable = if let TokenKind::Identifier(name) = self.peek() {
            if matches!(&self.tokens[self.pos + 1].kind, TokenKind::Eq) {
                let name = name.clone();
                self.advance(); // identifier
                self.advance(); // =
                Some(name)
            } else {
                None
            }
        } else {
            None
        };

        let start = self.parse_node_pattern()?;
        let mut steps = Vec::new();
        while matches!(
            self.peek(),
            TokenKind::Dash | TokenKind::Lt | TokenKind::LBracket
        ) {
            let relationship = self.parse_rel_pattern()?;
            let node = self.parse_node_pattern()?;
            steps.push(PatternStep { relationship, node });
        }
        Ok(PathPattern {
            path_variable,
            start,
            steps,
        })
    }

    fn parse_node_pattern(&mut self) -> CypherResult<NodePattern> {
        self.expect(&TokenKind::LParen, "'(' to start a node pattern")?;
        let variable = self.parse_optional_variable();
        let labels = self.parse_labels();
        let properties = self.parse_inline_properties()?;
        self.expect(&TokenKind::RParen, "')' to close a node pattern")?;
        Ok(NodePattern {
            variable,
            labels,
            properties,
        })
    }

    fn parse_rel_pattern(&mut self) -> CypherResult<RelPattern> {
        // Leading direction: `<-` (incoming) or `-` (out/undirected).
        let incoming = self.eat(&TokenKind::Lt);
        self.expect(&TokenKind::Dash, "'-' in a relationship pattern")?;

        let mut variable = None;
        let mut types = Vec::new();
        let mut var_length = None;
        let mut properties = None;
        if self.eat(&TokenKind::LBracket) {
            variable = self.parse_optional_variable();
            types = self.parse_rel_types();
            var_length = self.parse_var_length()?;
            properties = self.parse_inline_properties()?;
            self.expect(&TokenKind::RBracket, "']' to close a relationship pattern")?;
        }

        // Trailing direction: `->` (outgoing) or `-` (undirected).
        self.expect(&TokenKind::Dash, "'-' in a relationship pattern")?;
        let outgoing = self.eat(&TokenKind::Gt);

        let direction = match (incoming, outgoing) {
            (true, false) => Direction::Incoming,
            (false, true) => Direction::Outgoing,
            (false, false) => Direction::Undirected,
            (true, true) => return Err(self.error("relationship cannot point both directions")),
        };

        Ok(RelPattern {
            direction,
            variable,
            types,
            var_length,
            properties,
        })
    }

    /// `:A|B|C` relationship types (the `:` is optional after `[var`).
    fn parse_rel_types(&mut self) -> Vec<String> {
        let mut types = Vec::new();
        if self.eat(&TokenKind::Colon) {
            if let Some(name) = self.try_identifier() {
                types.push(name);
            }
            while self.eat(&TokenKind::Pipe) {
                // `|:T` and `|T` are both accepted.
                self.eat(&TokenKind::Colon);
                if let Some(name) = self.try_identifier() {
                    types.push(name);
                }
            }
        }
        types
    }

    /// `*`, `*n`, `*n..`, `*..m`, `*n..m`.
    fn parse_var_length(&mut self) -> CypherResult<Option<VarLength>> {
        if !self.eat(&TokenKind::Star) {
            return Ok(None);
        }
        let min = self.try_integer_bound();
        let mut max = min;
        if self.eat(&TokenKind::DotDot) {
            max = self.try_integer_bound();
        }
        // `*n` (no `..`) means exactly n; `*` means unbounded both ways.
        Ok(Some(VarLength { min, max }))
    }

    fn try_integer_bound(&mut self) -> Option<u64> {
        if let TokenKind::Integer(n) = self.peek() {
            let n = *n;
            self.advance();
            u64::try_from(n).ok()
        } else {
            None
        }
    }

    /// Zero or more `:Label` labels on a node pattern.
    fn parse_labels(&mut self) -> Vec<String> {
        let mut labels = Vec::new();
        while self.eat(&TokenKind::Colon) {
            if let Some(name) = self.try_identifier() {
                labels.push(name);
            } else {
                break;
            }
        }
        labels
    }

    /// `{k: v, ...}` inline property map on a node/relationship pattern.
    fn parse_inline_properties(&mut self) -> CypherResult<Option<Vec<(String, Expr)>>> {
        if !self.eat(&TokenKind::LBrace) {
            return Ok(None);
        }
        let entries = self.parse_map_entries()?;
        self.expect(&TokenKind::RBrace, "'}' to close a property map")?;
        Ok(Some(entries))
    }

    fn parse_optional_variable(&mut self) -> Option<String> {
        if matches!(self.peek(), TokenKind::Identifier(_)) {
            self.try_identifier()
        } else {
            None
        }
    }

    // --- expressions (precedence climbing) ----------------------------------

    fn parse_expr(&mut self) -> CypherResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_xor()?;
        while self.eat_keyword(Keyword::Or) {
            let rhs = self.parse_xor()?;
            lhs = binary(BinaryOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_xor(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.eat_keyword(Keyword::Xor) {
            let rhs = self.parse_and()?;
            lhs = binary(BinaryOp::Xor, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_not()?;
        while self.eat_keyword(Keyword::And) {
            let rhs = self.parse_not()?;
            lhs = binary(BinaryOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_not(&mut self) -> CypherResult<Expr> {
        if self.eat_keyword(Keyword::Not) {
            let operand = self.parse_not()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            })
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_additive()?;
        loop {
            // String/list predicates: STARTS WITH, ENDS WITH, CONTAINS, IN.
            if self.eat_keyword(Keyword::Starts) {
                self.expect_keyword(Keyword::With, "WITH after STARTS")?;
                let rhs = self.parse_additive()?;
                lhs = binary(BinaryOp::StartsWith, lhs, rhs);
                continue;
            }
            if self.eat_keyword(Keyword::Ends) {
                self.expect_keyword(Keyword::With, "WITH after ENDS")?;
                let rhs = self.parse_additive()?;
                lhs = binary(BinaryOp::EndsWith, lhs, rhs);
                continue;
            }
            if self.eat_keyword(Keyword::Contains) {
                let rhs = self.parse_additive()?;
                lhs = binary(BinaryOp::Contains, lhs, rhs);
                continue;
            }
            if self.eat_keyword(Keyword::In) {
                let rhs = self.parse_additive()?;
                lhs = binary(BinaryOp::In, lhs, rhs);
                continue;
            }
            // IS [NOT] NULL.
            if self.is_keyword(Keyword::Is) {
                self.advance();
                let negated = self.eat_keyword(Keyword::Not);
                self.expect_keyword(Keyword::Null, "NULL after IS [NOT]")?;
                lhs = Expr::IsNull {
                    operand: Box::new(lhs),
                    negated,
                };
                continue;
            }
            // Relational operators.
            let op = match self.peek() {
                TokenKind::Eq => BinaryOp::Equal,
                TokenKind::Neq => BinaryOp::NotEqual,
                TokenKind::Lt => BinaryOp::LessThan,
                TokenKind::Lte => BinaryOp::LessThanOrEqual,
                TokenKind::Gt => BinaryOp::GreaterThan,
                TokenKind::Gte => BinaryOp::GreaterThanOrEqual,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Dash => BinaryOp::Subtract,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> CypherResult<Expr> {
        let mut lhs = self.parse_power()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Modulo,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_power()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_power(&mut self) -> CypherResult<Expr> {
        let lhs = self.parse_unary()?;
        if self.eat(&TokenKind::Caret) {
            // Right-associative.
            let rhs = self.parse_power()?;
            Ok(binary(BinaryOp::Power, lhs, rhs))
        } else {
            Ok(lhs)
        }
    }

    fn parse_unary(&mut self) -> CypherResult<Expr> {
        if self.eat(&TokenKind::Dash) {
            let operand = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(operand),
            })
        } else if self.eat(&TokenKind::Plus) {
            // Unary plus is a no-op.
            self.parse_unary()
        } else {
            self.parse_postfix()
        }
    }

    /// Postfix chain: property access `.k` and indexing `[i]`.
    fn parse_postfix(&mut self) -> CypherResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(&TokenKind::Dot) {
                let key = self.parse_identifier("property key after '.'")?;
                expr = Expr::Property {
                    base: Box::new(expr),
                    key,
                };
            } else if self.eat(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RBracket, "']' to close an index")?;
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> CypherResult<Expr> {
        match self.peek().clone() {
            TokenKind::Integer(n) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::Integer(n)))
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::Float(f)))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::String(s)))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::Boolean(true)))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::Boolean(false)))
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.advance();
                Ok(Expr::Literal(PropertyValue::Null))
            }
            TokenKind::Keyword(Keyword::Count) => {
                // `count(*)` is the one keyword that doubles as a function.
                self.advance();
                self.expect(&TokenKind::LParen, "'(' after count")?;
                let expr = if self.eat(&TokenKind::Star) {
                    Expr::CountStar
                } else {
                    let distinct = self.eat_keyword(Keyword::Distinct);
                    let args = self.parse_call_args()?;
                    Expr::FunctionCall {
                        name: "count".to_string(),
                        distinct,
                        args,
                    }
                };
                self.expect(&TokenKind::RParen, "')' to close count(...)")?;
                Ok(expr)
            }
            TokenKind::Parameter(name) => {
                self.advance();
                Ok(Expr::Parameter(name))
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(
                    &TokenKind::RParen,
                    "')' to close a parenthesised expression",
                )?;
                Ok(inner)
            }
            TokenKind::LBracket => self.parse_list_literal(),
            TokenKind::LBrace => self.parse_map_literal(),
            TokenKind::Identifier(name) => {
                self.advance();
                // A function call if immediately followed by `(`.
                if matches!(self.peek(), TokenKind::LParen) {
                    self.advance();
                    let distinct = self.eat_keyword(Keyword::Distinct);
                    let args = self.parse_call_args()?;
                    self.expect(&TokenKind::RParen, "')' to close a function call")?;
                    Ok(Expr::FunctionCall {
                        name,
                        distinct,
                        args,
                    })
                } else {
                    Ok(Expr::Variable(name))
                }
            }
            _ => Err(self.error("expected an expression")),
        }
    }

    fn parse_call_args(&mut self) -> CypherResult<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(self.peek(), TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while self.eat(&TokenKind::Comma) {
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }

    fn parse_list_literal(&mut self) -> CypherResult<Expr> {
        self.expect(&TokenKind::LBracket, "'['")?;
        let mut elements = Vec::new();
        if !matches!(self.peek(), TokenKind::RBracket) {
            elements.push(self.parse_expr()?);
            while self.eat(&TokenKind::Comma) {
                elements.push(self.parse_expr()?);
            }
        }
        self.expect(&TokenKind::RBracket, "']' to close a list literal")?;
        Ok(Expr::List(elements))
    }

    fn parse_map_literal(&mut self) -> CypherResult<Expr> {
        self.expect(&TokenKind::LBrace, "'{'")?;
        let entries = self.parse_map_entries()?;
        self.expect(&TokenKind::RBrace, "'}' to close a map literal")?;
        Ok(Expr::Map(entries))
    }

    /// `key: value, key: value` map body (used by both map literals and inline
    /// property maps).
    fn parse_map_entries(&mut self) -> CypherResult<Vec<(String, Expr)>> {
        let mut entries = Vec::new();
        if matches!(self.peek(), TokenKind::RBrace) {
            return Ok(entries);
        }
        loop {
            let key = self.parse_map_key()?;
            self.expect(&TokenKind::Colon, "':' after a map key")?;
            let value = self.parse_expr()?;
            entries.push((key, value));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(entries)
    }

    /// A map key is an identifier (and Cypher allows a string key too).
    fn parse_map_key(&mut self) -> CypherResult<String> {
        match self.peek().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(s)
            }
            _ => Err(self.error("expected a map key")),
        }
    }

    // --- identifier helpers -------------------------------------------------

    /// Consume an identifier, erroring if the next token is not one.
    fn parse_identifier(&mut self, what: &str) -> CypherResult<String> {
        match self.peek().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(self.error(format!("expected {what}"))),
        }
    }

    /// Consume an identifier if present, returning `None` otherwise.
    fn try_identifier(&mut self) -> Option<String> {
        if let TokenKind::Identifier(name) = self.peek().clone() {
            self.advance();
            Some(name)
        } else {
            None
        }
    }
}

fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}
