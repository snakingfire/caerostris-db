//! Query side-effect accounting — the engine's `QueryStatistics` surface.
//!
//! # Why this exists (BUG-0006)
//!
//! The openCypher TCK asserts the *side effects* of a statement, not just its
//! result rows, via a Gherkin step:
//!
//! ```gherkin
//! Then the side effects should be:
//!   | +nodes      | 1 |
//!   | +properties | 2 |
//!   | -properties | 1 |
//!   | +labels     | 1 |
//! ```
//!
//! A large class of write scenarios (`CREATE`/`MERGE`/`SET`/`REMOVE`/`DELETE`
//! and many transaction scenarios) report outcomes that are **not observable
//! from the result set** — `CREATE (n) DELETE n` returns no rows yet must report
//! `+nodes 1 / -nodes 1`. Without an engine-exposed side-effect counter the TCK
//! adapter cannot assert these scenarios at all, so they would be structurally
//! unpassable and Cat. 4 = 100% (a GATE) would be unreachable.
//!
//! Neo4j satisfies the same TCK step with a `QueryStatistics` object the harness
//! reads; this type is caerostris-db's equivalent. See
//! `.project/decisions/0007-tck-side-effect-observability.md` and
//! `.project/decisions/0012-tck-side-effect-counting-semantics.md`.
//!
//! # Counting semantics (pinned)
//!
//! The categories mirror the eight the TCK emits. Each is a non-negative count
//! of *occurrences*, never a net delta:
//!
//! - A statement that creates then deletes a node reports `+nodes 1` **and**
//!   `-nodes 1` (not `nodes 0`).
//! - `+properties` / `-properties` count individual property writes/removals.
//!   Overwriting an existing property with a new value is one `+properties`;
//!   setting a property to `null` (the openCypher idiom for removal) is one
//!   `-properties`. Setting a property to the value it already holds is **not**
//!   counted (no-op), per the TCK's expected values for the pinned release.
//! - A category **absent** from a TCK side-effect table is asserted to be `0`.
//!   [`QueryStatistics::from_tck_side_effects`] therefore yields a fully
//!   specified statistics object (missing rows = zero), and equality compares
//!   every category.

use std::fmt;

/// Errors returned when parsing a TCK `Then the side effects should be:` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffectParseError {
    /// A table row was not of the form `| <category> | <count> |`.
    MalformedRow(String),
    /// The category column was not one of the recognised TCK categories.
    UnknownCategory(String),
    /// The count column was not a non-negative integer.
    InvalidCount { category: String, value: String },
    /// The same category appeared more than once in the table.
    DuplicateCategory(String),
}

impl fmt::Display for SideEffectParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedRow(row) => {
                write!(
                    f,
                    "malformed side-effect row (expected `| key | n |`): {row:?}"
                )
            }
            Self::UnknownCategory(cat) => {
                write!(f, "unknown TCK side-effect category: {cat:?}")
            }
            Self::InvalidCount { category, value } => {
                write!(
                    f,
                    "invalid count {value:?} for side-effect category {category:?}"
                )
            }
            Self::DuplicateCategory(cat) => {
                write!(f, "duplicate side-effect category in table: {cat:?}")
            }
        }
    }
}

impl std::error::Error for SideEffectParseError {}

/// The eight side-effect categories the openCypher TCK asserts, in the canonical
/// order they are emitted by the suite.
///
/// Kept as an explicit list so the counter, the parser, and the canonical
/// renderer stay in lock-step: adding a category to the engine means adding it
/// here once.
const CATEGORIES: [&str; 8] = [
    "+nodes",
    "-nodes",
    "+relationships",
    "-relationships",
    "+labels",
    "-labels",
    "+properties",
    "-properties",
];

/// Engine-reported side effects of executing a single openCypher statement.
///
/// This is the surface the TCK adapter reads to assert
/// `Then the side effects should be:` steps. Counts are occurrence counts (see
/// the module docs), never net deltas. A freshly [`new`](Self::new) value is all
/// zeroes — the correct report for a read-only statement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueryStatistics {
    nodes_created: u64,
    nodes_deleted: u64,
    relationships_created: u64,
    relationships_deleted: u64,
    labels_added: u64,
    labels_removed: u64,
    properties_set: u64,
    properties_removed: u64,
}

impl QueryStatistics {
    /// A zeroed counter — the side effects of a read-only statement.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` if the statement had no side effects (every category is zero).
    /// A read-only query reports `true`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `true` if the statement had at least one side effect. The negation of
    /// [`is_empty`](Self::is_empty); provided because adapter assertions read
    /// more naturally in the positive.
    #[must_use]
    pub fn contains_side_effects(&self) -> bool {
        !self.is_empty()
    }

    /// `true` if `self` reports exactly the same side effects as `expected`
    /// across every category. This is the comparison the TCK adapter performs;
    /// it is equivalent to `self == expected` and reads more clearly at the
    /// assertion site.
    #[must_use]
    pub fn matches_side_effects(&self, expected: &Self) -> bool {
        self == expected
    }

    // --- recorders (called by the executor as it applies a statement) -------

    /// Record that `n` nodes were created.
    pub fn record_nodes_created(&mut self, n: u64) {
        self.nodes_created += n;
    }

    /// Record that `n` nodes were deleted.
    pub fn record_nodes_deleted(&mut self, n: u64) {
        self.nodes_deleted += n;
    }

    /// Record that `n` relationships were created.
    pub fn record_relationships_created(&mut self, n: u64) {
        self.relationships_created += n;
    }

    /// Record that `n` relationships were deleted.
    pub fn record_relationships_deleted(&mut self, n: u64) {
        self.relationships_deleted += n;
    }

    /// Record that `n` labels were added to nodes.
    pub fn record_labels_added(&mut self, n: u64) {
        self.labels_added += n;
    }

    /// Record that `n` labels were removed from nodes.
    pub fn record_labels_removed(&mut self, n: u64) {
        self.labels_removed += n;
    }

    /// Record that `n` properties were set (created or overwritten).
    pub fn record_properties_set(&mut self, n: u64) {
        self.properties_set += n;
    }

    /// Record that `n` properties were removed.
    pub fn record_properties_removed(&mut self, n: u64) {
        self.properties_removed += n;
    }

    // --- accessors (read by the adapter / diagnostics) ----------------------

    /// Number of nodes created.
    #[must_use]
    pub fn nodes_created(&self) -> u64 {
        self.nodes_created
    }

    /// Number of nodes deleted.
    #[must_use]
    pub fn nodes_deleted(&self) -> u64 {
        self.nodes_deleted
    }

    /// Number of relationships created.
    #[must_use]
    pub fn relationships_created(&self) -> u64 {
        self.relationships_created
    }

    /// Number of relationships deleted.
    #[must_use]
    pub fn relationships_deleted(&self) -> u64 {
        self.relationships_deleted
    }

    /// Number of labels added.
    #[must_use]
    pub fn labels_added(&self) -> u64 {
        self.labels_added
    }

    /// Number of labels removed.
    #[must_use]
    pub fn labels_removed(&self) -> u64 {
        self.labels_removed
    }

    /// Number of properties set.
    #[must_use]
    pub fn properties_set(&self) -> u64 {
        self.properties_set
    }

    /// Number of properties removed.
    #[must_use]
    pub fn properties_removed(&self) -> u64 {
        self.properties_removed
    }

    /// Look up a category by its canonical TCK key (e.g. `"+nodes"`).
    fn category(&self, key: &str) -> Option<u64> {
        match key {
            "+nodes" => Some(self.nodes_created),
            "-nodes" => Some(self.nodes_deleted),
            "+relationships" => Some(self.relationships_created),
            "-relationships" => Some(self.relationships_deleted),
            "+labels" => Some(self.labels_added),
            "-labels" => Some(self.labels_removed),
            "+properties" => Some(self.properties_set),
            "-properties" => Some(self.properties_removed),
            _ => None,
        }
    }

    /// Set a category by its canonical TCK key. Returns `false` for an unknown
    /// key.
    fn set_category(&mut self, key: &str, value: u64) -> bool {
        match key {
            "+nodes" => self.nodes_created = value,
            "-nodes" => self.nodes_deleted = value,
            "+relationships" => self.relationships_created = value,
            "-relationships" => self.relationships_deleted = value,
            "+labels" => self.labels_added = value,
            "-labels" => self.labels_removed = value,
            "+properties" => self.properties_set = value,
            "-properties" => self.properties_removed = value,
            _ => return false,
        }
        true
    }

    /// Parse a TCK `Then the side effects should be:` table into the expected
    /// [`QueryStatistics`].
    ///
    /// `table` is the rows of the Gherkin data table, one per line, each of the
    /// form `| <category> | <count> |`. Surrounding pipes and whitespace are
    /// tolerated; blank lines are ignored. **Categories not present in the table
    /// are taken to be zero**, matching the TCK's convention, so the returned
    /// value is fully specified and can be compared with `==`.
    ///
    /// # Errors
    ///
    /// Returns [`SideEffectParseError`] if a row is malformed, names an unknown
    /// category, repeats a category, or carries a non-integer count.
    pub fn from_tck_side_effects(table: &str) -> Result<Self, SideEffectParseError> {
        let mut stats = Self::default();
        let mut seen: Vec<&str> = Vec::new();

        for raw in table.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            // Split on `|`; a well-formed row `| key | n |` yields empty first
            // and last fields plus the two cells.
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            // Expect: ["", key, count, ""].
            if cells.len() != 4 || !cells[0].is_empty() || !cells[3].is_empty() {
                return Err(SideEffectParseError::MalformedRow(raw.to_string()));
            }
            let key = cells[1];
            let count_str = cells[2];

            let canonical = CATEGORIES
                .iter()
                .find(|c| **c == key)
                .ok_or_else(|| SideEffectParseError::UnknownCategory(key.to_string()))?;

            if seen.contains(canonical) {
                return Err(SideEffectParseError::DuplicateCategory(key.to_string()));
            }
            seen.push(canonical);

            let count: u64 = count_str
                .parse()
                .map_err(|_| SideEffectParseError::InvalidCount {
                    category: key.to_string(),
                    value: count_str.to_string(),
                })?;

            // Unknown keys are already rejected above, so this always succeeds.
            stats.set_category(canonical, count);
        }

        Ok(stats)
    }

    /// Render these statistics as a canonical TCK side-effect table, one
    /// `| +category | n |` row per non-zero category in TCK order. An empty
    /// statistics object renders as the empty string. Used for diagnostics when
    /// an adapter assertion fails.
    #[must_use]
    pub fn to_tck_side_effects(&self) -> String {
        let mut out = String::new();
        for key in CATEGORIES {
            let count = self.category(key).unwrap_or(0);
            if count != 0 {
                out.push_str(&format!("| {key} | {count} |\n"));
            }
        }
        out
    }
}

impl fmt::Display for QueryStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered = self.to_tck_side_effects();
        if rendered.is_empty() {
            write!(f, "(no side effects)")
        } else {
            write!(f, "{}", rendered.trim_end())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty_and_has_no_side_effects() {
        let s = QueryStatistics::new();
        assert!(s.is_empty());
        assert!(!s.contains_side_effects());
        assert_eq!(s, QueryStatistics::default());
    }

    #[test]
    fn recorders_accumulate_each_category() {
        let mut s = QueryStatistics::new();
        s.record_nodes_created(2);
        s.record_nodes_created(1);
        s.record_nodes_deleted(1);
        s.record_relationships_created(3);
        s.record_relationships_deleted(1);
        s.record_labels_added(4);
        s.record_labels_removed(1);
        s.record_properties_set(5);
        s.record_properties_removed(2);

        assert_eq!(s.nodes_created(), 3);
        assert_eq!(s.nodes_deleted(), 1);
        assert_eq!(s.relationships_created(), 3);
        assert_eq!(s.relationships_deleted(), 1);
        assert_eq!(s.labels_added(), 4);
        assert_eq!(s.labels_removed(), 1);
        assert_eq!(s.properties_set(), 5);
        assert_eq!(s.properties_removed(), 2);
        assert!(s.contains_side_effects());
    }

    #[test]
    fn parses_every_category_with_surrounding_pipes_and_whitespace() {
        let table = "\
            | +nodes          | 1 |\n\
            | -nodes          | 2 |\n\
            | +relationships  | 3 |\n\
            | -relationships  | 4 |\n\
            | +labels         | 5 |\n\
            | -labels         | 6 |\n\
            | +properties     | 7 |\n\
            | -properties     | 8 |\n";
        let s = QueryStatistics::from_tck_side_effects(table).unwrap();
        assert_eq!(s.nodes_created(), 1);
        assert_eq!(s.nodes_deleted(), 2);
        assert_eq!(s.relationships_created(), 3);
        assert_eq!(s.relationships_deleted(), 4);
        assert_eq!(s.labels_added(), 5);
        assert_eq!(s.labels_removed(), 6);
        assert_eq!(s.properties_set(), 7);
        assert_eq!(s.properties_removed(), 8);
    }

    #[test]
    fn omitted_categories_are_zero() {
        let s = QueryStatistics::from_tck_side_effects("| +nodes | 1 |\n").unwrap();
        assert_eq!(s.nodes_created(), 1);
        // Everything else is implicitly zero per the TCK convention.
        assert_eq!(s.nodes_deleted(), 0);
        assert_eq!(s.relationships_created(), 0);
        assert_eq!(s.properties_set(), 0);
    }

    #[test]
    fn empty_table_parses_to_zero() {
        let s = QueryStatistics::from_tck_side_effects("\n   \n").unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn malformed_row_is_rejected() {
        assert_eq!(
            QueryStatistics::from_tck_side_effects("+nodes 1\n"),
            Err(SideEffectParseError::MalformedRow("+nodes 1".to_string())),
        );
        assert_eq!(
            QueryStatistics::from_tck_side_effects("| +nodes | 1 | extra |\n"),
            Err(SideEffectParseError::MalformedRow(
                "| +nodes | 1 | extra |".to_string()
            )),
        );
    }

    #[test]
    fn unknown_category_is_rejected() {
        assert_eq!(
            QueryStatistics::from_tck_side_effects("| +widgets | 1 |\n"),
            Err(SideEffectParseError::UnknownCategory(
                "+widgets".to_string()
            )),
        );
    }

    #[test]
    fn invalid_count_is_rejected() {
        assert_eq!(
            QueryStatistics::from_tck_side_effects("| +nodes | many |\n"),
            Err(SideEffectParseError::InvalidCount {
                category: "+nodes".to_string(),
                value: "many".to_string(),
            }),
        );
        // Negative counts are not valid (occurrence counts are non-negative).
        assert!(matches!(
            QueryStatistics::from_tck_side_effects("| -nodes | -1 |\n"),
            Err(SideEffectParseError::InvalidCount { .. }),
        ));
    }

    #[test]
    fn duplicate_category_is_rejected() {
        assert_eq!(
            QueryStatistics::from_tck_side_effects("| +nodes | 1 |\n| +nodes | 2 |\n"),
            Err(SideEffectParseError::DuplicateCategory(
                "+nodes".to_string()
            )),
        );
    }

    #[test]
    fn matches_side_effects_is_equality() {
        let expected = QueryStatistics::from_tck_side_effects("| +nodes | 1 |\n").unwrap();
        let mut actual = QueryStatistics::new();
        actual.record_nodes_created(1);
        assert!(actual.matches_side_effects(&expected));
        actual.record_nodes_created(1);
        assert!(!actual.matches_side_effects(&expected));
    }

    #[test]
    fn render_round_trips_through_parse() {
        let mut s = QueryStatistics::new();
        s.record_nodes_created(1);
        s.record_nodes_deleted(1);
        s.record_properties_set(2);
        let rendered = s.to_tck_side_effects();
        let reparsed = QueryStatistics::from_tck_side_effects(&rendered).unwrap();
        assert_eq!(s, reparsed);
        // TCK order: +nodes before -nodes before +properties.
        assert_eq!(
            rendered,
            "| +nodes | 1 |\n| -nodes | 1 |\n| +properties | 2 |\n"
        );
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(QueryStatistics::new().to_string(), "(no side effects)");
        let mut s = QueryStatistics::new();
        s.record_nodes_created(1);
        assert_eq!(s.to_string(), "| +nodes | 1 |");
    }

    #[test]
    fn parse_error_displays() {
        let err = QueryStatistics::from_tck_side_effects("oops\n").unwrap_err();
        assert!(err.to_string().contains("malformed"));
    }
}
