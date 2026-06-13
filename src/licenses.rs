//! License-manifest hygiene checks (rubric Cat. 12).
//!
//! Every dependency that ends up in `Cargo.lock` must be recorded in the
//! committed license manifest at `docs/licenses/manifest.toml`, with a
//! permissive SPDX identifier from an approved allow-list. This module provides
//! the pure parsing/checking logic; an integration test
//! (`tests/license_manifest.rs`) runs it against the real repository files so a
//! dependency added without a manifest entry — or carrying a non-permissive
//! license — fails CI.
//!
//! The check is intentionally self-contained (no `cargo-deny` needed at runtime)
//! so it works in any environment. `cargo-deny` is *additionally* wired into CI
//! (`deny.toml`) as defense in depth — see `docs/process/open-source-guardrails.md`.

use std::collections::BTreeSet;

/// SPDX identifiers permitted without steering sign-off.
///
/// Mirrors the "approved license families" list in
/// `docs/process/open-source-guardrails.md`. Copyleft / source-available
/// licenses are deliberately excluded: they require a recorded steering
/// decision before use and must not pass this automated check silently.
pub const APPROVED_SPDX: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "MPL-2.0",
    "CC0-1.0",
    "Unlicense",
    "Unicode-3.0",
    "Zlib",
];

/// A single dependency parsed out of `Cargo.lock`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedCrate {
    /// Crate name as it appears in `Cargo.lock`.
    pub name: String,
    /// Resolved version.
    pub version: String,
}

/// A recorded entry in the license manifest (`docs/licenses/manifest.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Crate/package name.
    pub name: String,
    /// Recorded SPDX license expression (single-license form for the allow-list
    /// check; OR-expressions are split on `OR`).
    pub spdx: String,
}

/// A problem found while checking the lockfile against the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseViolation {
    /// A dependency in `Cargo.lock` has no entry in the manifest.
    MissingManifestEntry { crate_name: String },
    /// A manifest entry carries an SPDX id outside the approved allow-list.
    NonPermissiveLicense { crate_name: String, spdx: String },
}

impl std::fmt::Display for LicenseViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseViolation::MissingManifestEntry { crate_name } => write!(
                f,
                "dependency `{crate_name}` is in Cargo.lock but missing from \
                 docs/licenses/manifest.toml — add an entry with its SPDX id \
                 and a permissive-compatibility note"
            ),
            LicenseViolation::NonPermissiveLicense { crate_name, spdx } => write!(
                f,
                "dependency `{crate_name}` is recorded with license `{spdx}`, \
                 which is not in the approved allow-list — get steering sign-off \
                 and record the decision before adding it"
            ),
        }
    }
}

/// Returns `true` iff the SPDX license expression is satisfiable using only
/// licenses on the approved allow-list ([`APPROVED_SPDX`]).
///
/// The expression is parsed and evaluated honoring SPDX operator precedence and
/// grouping (SPDX spec, Annex D):
///
/// - `OR` (disjunction, *lowest* precedence): permissive iff *any* operand is.
/// - `AND` (conjunction, binds tighter than `OR`): permissive iff *all* operands
///   are — an `AND` operand is a license you are *required* to comply with.
/// - `(...)` parentheses override the default precedence.
/// - `WITH <exception>` (license exception, binds tightest) is treated as a
///   single opaque token; since no exception-bearing identifier is on the
///   allow-list it is conservatively rejected.
/// - The legacy crates.io `A/B` slash form is normalized to `A OR B`.
///
/// Token matching is case-insensitive. Malformed expressions (unbalanced
/// parentheses, empty or dangling operands) are conservatively rejected
/// (`false`) — we never guess in the permissive direction.
///
/// This fixes BUG-0008: the previous substring heuristic checked for ` OR `
/// before ` AND `, so `(MIT OR Apache-2.0) AND GPL-3.0` was misclassified as a
/// pure disjunction and its required copyleft `AND` operand was ignored.
#[must_use]
pub fn is_permissive(spdx: &str) -> bool {
    let normalized = spdx.replace('/', " OR ");
    let tokens = match spdx_tokenize(&normalized) {
        Some(t) => t,
        None => return false, // unrecognized character → reject conservatively
    };
    let mut parser = SpdxParser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
    };
    match parser.parse_or() {
        // A well-formed parse that consumed the whole expression.
        Some(permissive) if parser.pos == tokens.len() => permissive,
        _ => false, // dangling tokens / unbalanced parens / empty → reject
    }
}

/// A lexical token of an SPDX license expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SpdxToken {
    Or,
    And,
    With,
    LParen,
    RParen,
    /// A license / exception identifier (already lower-cased for matching).
    Ident(String),
}

/// Split an SPDX expression into tokens. Returns `None` if a character outside
/// the recognized set (idents, whitespace, parentheses) appears, so callers can
/// reject malformed input rather than silently dropping it.
fn spdx_tokenize(input: &str) -> Option<Vec<SpdxToken>> {
    let mut tokens = Vec::new();
    let mut ident = String::new();

    // Flush an accumulated identifier, classifying reserved words.
    let flush = |ident: &mut String, tokens: &mut Vec<SpdxToken>| {
        if ident.is_empty() {
            return;
        }
        match ident.as_str() {
            "or" => tokens.push(SpdxToken::Or),
            "and" => tokens.push(SpdxToken::And),
            "with" => tokens.push(SpdxToken::With),
            _ => tokens.push(SpdxToken::Ident(std::mem::take(ident))),
        }
        ident.clear();
    };

    for ch in input.chars() {
        match ch {
            '(' | ')' => {
                flush(&mut ident, &mut tokens);
                tokens.push(if ch == '(' {
                    SpdxToken::LParen
                } else {
                    SpdxToken::RParen
                });
            }
            c if c.is_whitespace() => flush(&mut ident, &mut tokens),
            // SPDX ids: letters, digits, '.', '-', '+'. Reject anything else.
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+') => {
                ident.push(c.to_ascii_lowercase());
            }
            _ => return None,
        }
    }
    flush(&mut ident, &mut tokens);
    Some(tokens)
}

/// Maximum parenthesis-nesting depth the recursive-descent evaluator will
/// descend into before refusing to continue. Real SPDX expressions nest a
/// handful of levels at most; a far higher bound than any legitimate
/// expression needs, yet low enough that the bounded recursion cannot overflow
/// the stack. Input nested deeper than this is rejected conservatively
/// (`false`) rather than aborting the process — fixes BUG-0015, where ~200k
/// nested parens overflowed the stack and SIGABRT-ed the whole binary.
const MAX_PAREN_DEPTH: usize = 64;

/// Recursive-descent evaluator over SPDX tokens. Each `parse_*` returns the
/// permissive verdict of the sub-expression it consumed, or `None` on a parse
/// error (which the caller turns into a conservative `false`).
struct SpdxParser<'a> {
    tokens: &'a [SpdxToken],
    pos: usize,
    /// Current parenthesis-nesting depth, bounded by [`MAX_PAREN_DEPTH`] so a
    /// pathologically deep expression is rejected instead of overflowing the
    /// stack (BUG-0015).
    depth: usize,
}

impl SpdxParser<'_> {
    fn peek(&self) -> Option<&SpdxToken> {
        self.tokens.get(self.pos)
    }

    /// `or_expr := and_expr ( "OR" and_expr )*` — permissive iff any operand is.
    fn parse_or(&mut self) -> Option<bool> {
        let mut acc = self.parse_and()?;
        while matches!(self.peek(), Some(SpdxToken::Or)) {
            self.pos += 1;
            let rhs = self.parse_and()?;
            acc = acc || rhs;
        }
        Some(acc)
    }

    /// `and_expr := with_expr ( "AND" with_expr )*` — permissive iff all are.
    fn parse_and(&mut self) -> Option<bool> {
        let mut acc = self.parse_with()?;
        while matches!(self.peek(), Some(SpdxToken::And)) {
            self.pos += 1;
            let rhs = self.parse_with()?;
            acc = acc && rhs;
        }
        Some(acc)
    }

    /// `with_expr := atom ( "WITH" ident )?` — an exception-bearing token is not
    /// on the allow-list, so it is never permissive.
    fn parse_with(&mut self) -> Option<bool> {
        let lhs = self.parse_atom()?;
        if matches!(self.peek(), Some(SpdxToken::With)) {
            self.pos += 1;
            // The exception must be a bare identifier.
            match self.peek() {
                Some(SpdxToken::Ident(_)) => {
                    self.pos += 1;
                    Some(false) // no `<license> WITH <exception>` is approved
                }
                _ => None, // dangling WITH → parse error
            }
        } else {
            Some(lhs)
        }
    }

    /// `atom := "(" or_expr ")" | ident`
    fn parse_atom(&mut self) -> Option<bool> {
        match self.peek() {
            Some(SpdxToken::LParen) => {
                // Bound the recursion: refuse to descend past MAX_PAREN_DEPTH so
                // a pathologically deep expression is rejected conservatively
                // (`false`) instead of overflowing the stack (BUG-0015).
                if self.depth >= MAX_PAREN_DEPTH {
                    return None;
                }
                self.pos += 1;
                self.depth += 1;
                let inner = self.parse_or()?;
                self.depth -= 1;
                if matches!(self.peek(), Some(SpdxToken::RParen)) {
                    self.pos += 1;
                    Some(inner)
                } else {
                    None // unbalanced paren
                }
            }
            Some(SpdxToken::Ident(tok)) => {
                let approved = APPROVED_SPDX.iter().any(|a| a.eq_ignore_ascii_case(tok));
                self.pos += 1;
                Some(approved)
            }
            _ => None, // operator/paren where an atom was expected, or end-of-input
        }
    }
}

/// If `line` is a `key = value` assignment for `key`, return its unquoted value.
///
/// Splits on the **first** `=` and trims whitespace on both sides, so the key is
/// matched regardless of the spacing around `=` — single-space (`name = "x"`),
/// aligned (`name    = "x"`), tab-separated, or no-space (`name="x"`) all parse
/// identically. The key comparison is exact on the trimmed left-hand side, so
/// look-alike keys (`namespace`, `spdx_note`) are not mistaken for `name`/`spdx`.
/// Surrounding double quotes are stripped from the value (TOML string form).
///
/// Returning the spacing to the rigid `strip_prefix("name = ")` form is what
/// caused BUG-0014: a dependency recorded in the manifest's own documented
/// aligned style was silently dropped, failing the license gate *open*.
fn parse_key_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (lhs, rhs) = line.split_once('=')?;
    if lhs.trim() != key {
        return None;
    }
    Some(rhs.trim().trim_matches('"'))
}

/// Parse the crate entries out of a `Cargo.lock` file body.
///
/// `own_crates` are names of workspace members (e.g. the crate itself), which
/// are skipped — they are not third-party dependencies needing a license entry.
#[must_use]
pub fn parse_lockfile(contents: &str, own_crates: &[&str]) -> Vec<LockedCrate> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut in_package = false;

    let flush =
        |name: &mut Option<String>, version: &mut Option<String>, out: &mut Vec<LockedCrate>| {
            if let (Some(n), Some(v)) = (name.take(), version.take()) {
                if !own_crates.contains(&n.as_str()) {
                    out.push(LockedCrate {
                        name: n,
                        version: v,
                    });
                }
            } else {
                *name = None;
                *version = None;
            }
        };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            // Starting a new package block — flush any complete previous one.
            flush(&mut name, &mut version, &mut out);
            in_package = true;
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(value) = parse_key_value(trimmed, "name") {
            name = Some(value.to_string());
        } else if let Some(value) = parse_key_value(trimmed, "version") {
            version = Some(value.to_string());
        }
    }
    // Flush the final package block.
    flush(&mut name, &mut version, &mut out);
    out
}

/// Parse the license manifest body.
///
/// The manifest is a TOML-ish table list, but to avoid a TOML dependency we
/// parse the small, well-defined subset we write ourselves: blocks introduced
/// by `[[dependency]]` with `name = "..."` and `spdx = "..."` keys. Key spacing
/// is irrelevant — single-space, aligned, tab, and no-space forms all parse the
/// same (see [`parse_key_value`]) so the gate cannot fail open on aligned style.
#[must_use]
pub fn parse_manifest(contents: &str) -> Vec<ManifestEntry> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut spdx: Option<String> = None;
    let mut in_dep = false;

    let flush =
        |name: &mut Option<String>, spdx: &mut Option<String>, out: &mut Vec<ManifestEntry>| {
            if let (Some(n), Some(s)) = (name.clone(), spdx.clone()) {
                out.push(ManifestEntry { name: n, spdx: s });
            }
            *name = None;
            *spdx = None;
        };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[dependency]]" {
            flush(&mut name, &mut spdx, &mut out);
            in_dep = true;
            continue;
        }
        if !in_dep {
            continue;
        }
        if let Some(value) = parse_key_value(trimmed, "name") {
            name = Some(value.to_string());
        } else if let Some(value) = parse_key_value(trimmed, "spdx") {
            spdx = Some(value.to_string());
        }
    }
    flush(&mut name, &mut spdx, &mut out);
    out
}

/// Check every locked dependency against the manifest and the allow-list.
///
/// Returns the (possibly empty) set of violations. An empty result means the
/// manifest is complete and every recorded license is permissive.
#[must_use]
pub fn check(locked: &[LockedCrate], manifest: &[ManifestEntry]) -> Vec<LicenseViolation> {
    let mut violations = Vec::new();

    let manifest_names: BTreeSet<&str> = manifest.iter().map(|e| e.name.as_str()).collect();

    for dep in locked {
        if !manifest_names.contains(dep.name.as_str()) {
            violations.push(LicenseViolation::MissingManifestEntry {
                crate_name: dep.name.clone(),
            });
        }
    }

    for entry in manifest {
        if !is_permissive(&entry.spdx) {
            violations.push(LicenseViolation::NonPermissiveLicense {
                crate_name: entry.name.clone(),
                spdx: entry.spdx.clone(),
            });
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_single_licenses_are_permissive() {
        assert!(is_permissive("MIT"));
        assert!(is_permissive("Apache-2.0"));
        assert!(is_permissive("BSD-3-Clause"));
        assert!(is_permissive("MPL-2.0"));
    }

    #[test]
    fn copyleft_licenses_are_not_permissive() {
        assert!(!is_permissive("GPL-3.0"));
        assert!(!is_permissive("AGPL-3.0"));
        assert!(!is_permissive("LGPL-2.1"));
        assert!(!is_permissive("SSPL-1.0"));
        assert!(!is_permissive("BUSL-1.1"));
    }

    #[test]
    fn or_expression_is_permissive_if_any_token_approved() {
        assert!(is_permissive("MIT OR Apache-2.0"));
        assert!(is_permissive("GPL-3.0 OR MIT"));
        assert!(is_permissive("MIT/Apache-2.0")); // legacy slash form
    }

    #[test]
    fn or_expression_is_not_permissive_if_no_token_approved() {
        assert!(!is_permissive("GPL-3.0 OR AGPL-3.0"));
    }

    #[test]
    fn and_expression_requires_all_tokens_approved() {
        assert!(is_permissive("MIT AND Apache-2.0"));
        assert!(!is_permissive("MIT AND GPL-3.0"));
    }

    // --- BUG-0008: mixed AND/OR conjunctions ---------------------------------

    #[test]
    fn parenthesized_or_anded_with_copyleft_is_not_permissive() {
        // The reported bug: a permissive disjunction AND a copyleft component.
        // GPL-3.0 is a *required* conjunct, so the whole expression is not
        // permissive — even though `MIT OR Apache-2.0` on its own would be.
        assert!(!is_permissive("(MIT OR Apache-2.0) AND GPL-3.0"));
    }

    #[test]
    fn parenthesized_disjuncts_anded_are_permissive_when_each_satisfiable() {
        // Each conjunct is a disjunction with at least one approved token:
        //   (MIT OR GPL-3.0)  -> MIT approved
        //   Apache-2.0        -> approved
        // so the conjunction is satisfiable by approved tokens and is permissive.
        assert!(is_permissive("(MIT OR GPL-3.0) AND Apache-2.0"));
    }

    #[test]
    fn parenthesized_disjuncts_anded_are_not_permissive_when_one_unsatisfiable() {
        // (MIT OR Apache-2.0) is satisfiable, but (GPL-3.0 OR AGPL-3.0) is not:
        // no approved token satisfies the second conjunct.
        assert!(!is_permissive(
            "(MIT OR Apache-2.0) AND (GPL-3.0 OR AGPL-3.0)"
        ));
    }

    #[test]
    fn and_binds_tighter_than_or_without_parens() {
        // SPDX precedence: AND binds tighter than OR, so
        //   MIT OR Apache-2.0 AND GPL-3.0  ==  MIT OR (Apache-2.0 AND GPL-3.0)
        // MIT alone satisfies the disjunction -> permissive.
        assert!(is_permissive("MIT OR Apache-2.0 AND GPL-3.0"));
        //   GPL-3.0 OR MIT AND BSD-3-Clause  ==  GPL-3.0 OR (MIT AND BSD-3-Clause)
        // (MIT AND BSD-3-Clause) is fully approved -> permissive.
        assert!(is_permissive("GPL-3.0 OR MIT AND BSD-3-Clause"));
        //   GPL-3.0 AND MIT OR LGPL-2.1  ==  (GPL-3.0 AND MIT) OR LGPL-2.1
        // neither operand of the top-level OR is permissive -> not permissive.
        assert!(!is_permissive("GPL-3.0 AND MIT OR LGPL-2.1"));
    }

    #[test]
    fn nested_parentheses_are_honored() {
        // ((MIT OR GPL-3.0) AND Apache-2.0) OR SSPL-1.0
        // left operand is permissive (see above) -> whole expression permissive.
        assert!(is_permissive(
            "((MIT OR GPL-3.0) AND Apache-2.0) OR SSPL-1.0"
        ));
        // (MIT AND (GPL-3.0 OR AGPL-3.0)) -> second conjunct unsatisfiable -> not permissive.
        assert!(!is_permissive("MIT AND (GPL-3.0 OR AGPL-3.0)"));
    }

    // --- BUG-0015: parenthesis-nesting depth cap (no unbounded recursion) ----

    /// Wrap `inner` in `depth` layers of parentheses: `(((... inner ...)))`.
    fn nest(inner: &str, depth: usize) -> String {
        format!("{}{inner}{}", "(".repeat(depth), ")".repeat(depth))
    }

    #[test]
    fn deeply_nested_parens_are_rejected_without_aborting() {
        // The reported repro: 200_000 nested parens overflowed the stack and
        // aborted the process (SIGABRT). It must now return a conservative
        // `false` and the process must survive.
        let expr = nest("MIT", 200_000);
        assert!(!is_permissive(&expr));
        // Reaching here at all proves we did not overflow the stack.
    }

    #[test]
    fn nesting_at_the_depth_cap_still_classifies_correctly() {
        // A permissive identifier wrapped to exactly the maximum allowed depth
        // must still be honored — the cap rejects only *beyond* the bound.
        let at_cap = nest("MIT", MAX_PAREN_DEPTH);
        assert!(is_permissive(&at_cap));
        // A copyleft identifier at the cap stays non-permissive.
        let at_cap_copyleft = nest("GPL-3.0", MAX_PAREN_DEPTH);
        assert!(!is_permissive(&at_cap_copyleft));
    }

    #[test]
    fn nesting_one_past_the_cap_is_conservatively_rejected() {
        // One level deeper than the cap is rejected conservatively (`false`),
        // even for an otherwise-permissive identifier — we never guess
        // permissive when we refuse to evaluate.
        let past_cap = nest("MIT", MAX_PAREN_DEPTH + 1);
        assert!(!is_permissive(&past_cap));
    }

    #[test]
    fn modest_realistic_nesting_is_honored() {
        // Realistic SPDX expressions never nest beyond a handful of levels;
        // depths well within the cap must classify exactly as the un-nested
        // expression would.
        for depth in [1usize, 4, 8, 16] {
            assert!(
                is_permissive(&nest("MIT", depth)),
                "permissive ident at depth {depth} should be permissive"
            );
            assert!(
                !is_permissive(&nest("GPL-3.0", depth)),
                "copyleft ident at depth {depth} should not be permissive"
            );
            // A nested disjunction is still evaluated under the precedence rules.
            assert!(
                is_permissive(&nest("MIT OR GPL-3.0", depth)),
                "nested disjunction with an approved branch should be permissive"
            );
        }
    }

    #[test]
    fn with_clause_is_treated_as_a_single_token() {
        // SPDX `WITH` (license exception) binds tightest. We do not approve any
        // exception-bearing token by default, so it is conservatively rejected,
        // and an AND with it stays rejected.
        assert!(!is_permissive("Apache-2.0 WITH LLVM-exception"));
        assert!(!is_permissive("MIT AND Apache-2.0 WITH LLVM-exception"));
        // But an OR where the other branch is plain-approved still passes.
        assert!(is_permissive("MIT OR Apache-2.0 WITH LLVM-exception"));
    }

    #[test]
    fn malformed_expressions_are_conservatively_rejected() {
        // Unbalanced parentheses, empty operands, or dangling operators must not
        // be silently classified permissive.
        assert!(!is_permissive(""));
        assert!(!is_permissive("(MIT"));
        assert!(!is_permissive("MIT)"));
        assert!(!is_permissive("MIT OR"));
        assert!(!is_permissive("AND MIT"));
        assert!(!is_permissive("()"));
        assert!(!is_permissive("MIT AND AND Apache-2.0"));
    }

    #[test]
    fn whitespace_and_case_are_normalized() {
        assert!(is_permissive("  mit   or   apache-2.0  "));
        assert!(is_permissive("(  MIT  )  and  (  apache-2.0  )"));
        assert!(!is_permissive("(mit or apache-2.0) and gpl-3.0"));
    }

    #[test]
    fn parse_lockfile_skips_own_crate() {
        let lock = r#"
version = 4

[[package]]
name = "caerostris-db"
version = "0.0.0"
"#;
        let parsed = parse_lockfile(lock, &["caerostris-db"]);
        assert!(parsed.is_empty(), "own crate should be skipped: {parsed:?}");
    }

    #[test]
    fn parse_lockfile_extracts_dependencies() {
        let lock = r#"
version = 4

[[package]]
name = "caerostris-db"
version = "0.0.0"
dependencies = [
 "serde",
]

[[package]]
name = "serde"
version = "1.0.210"

[[package]]
name = "thiserror"
version = "1.0.64"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        let parsed = parse_lockfile(lock, &["caerostris-db"]);
        assert_eq!(
            parsed,
            vec![
                LockedCrate {
                    name: "serde".into(),
                    version: "1.0.210".into()
                },
                LockedCrate {
                    name: "thiserror".into(),
                    version: "1.0.64".into()
                },
            ]
        );
    }

    #[test]
    fn parse_manifest_extracts_entries() {
        let manifest = r#"
# License manifest
[[dependency]]
name = "serde"
version = "1.0.210"
spdx = "MIT OR Apache-2.0"
note = "Permissive; ubiquitous."

[[dependency]]
name = "thiserror"
version = "1.0.64"
spdx = "MIT OR Apache-2.0"
note = "Permissive."
"#;
        let parsed = parse_manifest(manifest);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "serde");
        assert_eq!(parsed[0].spdx, "MIT OR Apache-2.0");
        assert_eq!(parsed[1].name, "thiserror");
    }

    /// Regression for BUG-0014: the manifest's own documented format example
    /// uses *aligned* keys (`name    = "..."`), and taplo formats TOML that way.
    /// The old `strip_prefix("name = ")` required exactly one space and silently
    /// dropped such entries — a fail-open license gate. Parsing must be robust to
    /// arbitrary whitespace around `=`.
    #[test]
    fn parse_manifest_handles_aligned_key_whitespace() {
        let manifest = r#"
# License manifest
[[dependency]]
name    = "serde"
version = "1.0.210"
spdx    = "MIT OR Apache-2.0"
note    = "Permissive; ubiquitous."
"#;
        let parsed = parse_manifest(manifest);
        assert_eq!(
            parsed,
            vec![ManifestEntry {
                name: "serde".into(),
                spdx: "MIT OR Apache-2.0".into(),
            }],
            "aligned-key block must parse, not be silently dropped"
        );
    }

    /// The fail-open hazard end-to-end: an aligned-key dependency carrying a
    /// non-permissive (copyleft) license must be parsed AND flagged by `check`,
    /// proving the license gate no longer passes it silently. This is the exact
    /// acceptance criterion for BUG-0014.
    #[test]
    fn parse_manifest_aligned_non_permissive_entry_is_flagged_by_check() {
        let manifest_src = r#"
[[dependency]]
name    = "copyleft-crate"
version = "1.0.0"
spdx    = "GPL-3.0"
note    = "Recorded in aligned style."
"#;
        let manifest = parse_manifest(manifest_src);
        // First: it must actually be parsed (the bug dropped it entirely).
        assert_eq!(manifest.len(), 1, "aligned entry must be parsed");

        let locked = vec![LockedCrate {
            name: "copyleft-crate".into(),
            version: "1.0.0".into(),
        }];
        let violations = check(&locked, &manifest);
        assert_eq!(
            violations,
            vec![LicenseViolation::NonPermissiveLicense {
                crate_name: "copyleft-crate".into(),
                spdx: "GPL-3.0".into()
            }],
            "gate must flag the non-permissive aligned entry, not fail open"
        );
    }

    /// Whitespace handling must be symmetric: no spaces around `=`, and a tab as
    /// the separator, must both parse identically to the single-space form.
    #[test]
    fn parse_manifest_handles_no_space_and_tab_around_equals() {
        let manifest = "[[dependency]]\nname=\"a\"\nspdx\t=\t\"MIT\"\n";
        let parsed = parse_manifest(manifest);
        assert_eq!(
            parsed,
            vec![ManifestEntry {
                name: "a".into(),
                spdx: "MIT".into(),
            }]
        );
    }

    /// Keys that merely *start with* `name`/`spdx` (e.g. `namespace`,
    /// `spdx_note`) must NOT be mistaken for the real keys — splitting on the
    /// first `=` must compare the trimmed key exactly.
    #[test]
    fn parse_manifest_does_not_match_lookalike_keys() {
        let manifest = r#"
[[dependency]]
namespace = "not-a-name"
spdx_note = "not-a-license"
name = "real"
spdx = "MIT"
"#;
        let parsed = parse_manifest(manifest);
        assert_eq!(
            parsed,
            vec![ManifestEntry {
                name: "real".into(),
                spdx: "MIT".into(),
            }],
            "look-alike keys must not be parsed as name/spdx"
        );
    }

    #[test]
    fn check_flags_dependency_missing_from_manifest() {
        let locked = vec![LockedCrate {
            name: "serde".into(),
            version: "1.0.210".into(),
        }];
        let manifest = vec![]; // empty — serde not recorded
        let violations = check(&locked, &manifest);
        assert_eq!(
            violations,
            vec![LicenseViolation::MissingManifestEntry {
                crate_name: "serde".into()
            }]
        );
    }

    #[test]
    fn check_flags_non_permissive_manifest_entry() {
        let locked = vec![LockedCrate {
            name: "copyleft-crate".into(),
            version: "1.0.0".into(),
        }];
        let manifest = vec![ManifestEntry {
            name: "copyleft-crate".into(),
            spdx: "GPL-3.0".into(),
        }];
        let violations = check(&locked, &manifest);
        assert_eq!(
            violations,
            vec![LicenseViolation::NonPermissiveLicense {
                crate_name: "copyleft-crate".into(),
                spdx: "GPL-3.0".into()
            }]
        );
    }

    #[test]
    fn check_passes_when_all_deps_recorded_and_permissive() {
        let locked = vec![LockedCrate {
            name: "serde".into(),
            version: "1.0.210".into(),
        }];
        let manifest = vec![ManifestEntry {
            name: "serde".into(),
            spdx: "MIT OR Apache-2.0".into(),
        }];
        assert!(check(&locked, &manifest).is_empty());
    }

    #[test]
    fn violation_display_is_actionable() {
        let v = LicenseViolation::MissingManifestEntry {
            crate_name: "serde".into(),
        };
        let msg = v.to_string();
        assert!(msg.contains("serde"));
        assert!(msg.contains("manifest.toml"));
    }
}
