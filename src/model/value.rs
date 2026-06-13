//! The openCypher property value type system.
//!
//! [`PropertyValue`] models every value an openCypher expression can produce
//! that may be *stored on a node or relationship property*: the scalar set
//! (null, boolean, integer, float, string) and the container set (list, map).
//! It is the logical value type the engine reasons about; the on-object byte
//! encoding (SPIKE-0003) serialises to/from it but does not constrain it.
//!
//! # Two notions of "equal"
//!
//! openCypher distinguishes **value equality** (the `=` operator) from
//! **structural identity** (used by `DISTINCT`, grouping keys, and list/map
//! membership). They disagree on `null` and `NaN`:
//!
//! | comparison        | `=` operator ([`cypher_equal`]) | identity ([`PartialEq`]) |
//! |-------------------|---------------------------------|--------------------------|
//! | `null = null`     | `null` (unknown)                | equal                    |
//! | `null = 1`        | `null` (unknown)                | not equal                |
//! | `NaN = NaN`       | `false`                         | equal (same value)       |
//! | `1 = 1.0`         | `true`                          | not equal (Int ≠ Float)  |
//!
//! - [`cypher_equal`](PropertyValue::cypher_equal) implements the `=`
//!   *operator*: it is **ternary** (returns `Option<bool>`, where `None` is the
//!   openCypher `null`), propagates `null`, compares integers and floats by
//!   mathematical value, and follows IEEE for `NaN` (`NaN = NaN` is `false`).
//! - The derived [`PartialEq`] implements **structural identity** for use as a
//!   `DISTINCT` / grouping / membership key: `null` is identical to `null`, a
//!   value is never identical across distinct type tags (`Integer(1)` is *not*
//!   identical to `Float(1.0)`), and two `NaN`s are identical (so `DISTINCT`
//!   collapses them). `PropertyValue` deliberately does **not** implement `Eq`
//!   because `f64` is only `PartialEq`.
//!
//! For `ORDER BY`, [`cypher_order`](PropertyValue::cypher_order) provides a
//! **total** orderability relation over all values and all types.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A property value in the openCypher type system.
///
/// Covers the openCypher *property types* — the values that may be stored on a
/// node or relationship. Graph reference types (node, relationship, path) are
/// *not* property values in openCypher and so are intentionally absent here;
/// they belong to the query runtime, not the stored model.
///
/// Integers are `i64` and floats are `f64`, matching openCypher's `INTEGER` and
/// `FLOAT`. Lists are heterogeneous ordered sequences; maps are string-keyed
/// and use a [`BTreeMap`] so iteration order (and therefore serialisation) is
/// deterministic regardless of insertion order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    /// The openCypher `null` — absence of a value.
    Null,
    /// A boolean (`true` / `false`).
    Boolean(bool),
    /// A 64-bit signed integer (openCypher `INTEGER`).
    Integer(i64),
    /// A 64-bit IEEE-754 float (openCypher `FLOAT`).
    Float(f64),
    /// A UTF-8 string.
    String(String),
    /// An ordered, heterogeneous list.
    List(Vec<PropertyValue>),
    /// A string-keyed map. Backed by a [`BTreeMap`] for deterministic ordering.
    Map(BTreeMap<String, PropertyValue>),
}

impl PropertyValue {
    /// `true` if this value is [`PropertyValue::Null`].
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, PropertyValue::Null)
    }

    /// The openCypher type name of this value, as the TCK and error messages
    /// spell it (`"Null"`, `"Boolean"`, `"Integer"`, `"Float"`, `"String"`,
    /// `"List"`, `"Map"`).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            PropertyValue::Null => "Null",
            PropertyValue::Boolean(_) => "Boolean",
            PropertyValue::Integer(_) => "Integer",
            PropertyValue::Float(_) => "Float",
            PropertyValue::String(_) => "String",
            PropertyValue::List(_) => "List",
            PropertyValue::Map(_) => "Map",
        }
    }

    /// The openCypher `=` operator: **ternary** value equality.
    ///
    /// Returns:
    /// - `None` — the openCypher `null` result. Produced whenever either
    ///   operand is `null`, *or* when a contained element makes a list/map
    ///   comparison indeterminate (a `null` element where the lists are
    ///   otherwise equal in length and shape).
    /// - `Some(true)` / `Some(false)` — a definite boolean result.
    ///
    /// Semantics:
    /// - `null` on either side ⇒ `None` (unknown), including `null = null`.
    /// - `Integer` and `Float` compare by **mathematical value**
    ///   (`1 = 1.0` ⇒ `Some(true)`), with `NaN` never equal to anything
    ///   (`NaN = NaN` ⇒ `Some(false)`), per IEEE-754.
    /// - Different non-null types (other than the integer/float pair) are
    ///   never equal ⇒ `Some(false)`.
    /// - Lists compare element-wise: differing lengths ⇒ `Some(false)`; equal
    ///   length with every element definitely equal ⇒ `Some(true)`; any element
    ///   pair definitely unequal ⇒ `Some(false)`; otherwise (a `null`/unknown
    ///   element pair, no definite inequality) ⇒ `None`.
    /// - Maps compare on key sets then values: unequal key sets ⇒ `Some(false)`
    ///   (a definite structural difference, even if a value is `null`); equal
    ///   key sets fold their per-key value comparisons like lists.
    #[must_use]
    pub fn cypher_equal(&self, other: &PropertyValue) -> Option<bool> {
        use PropertyValue::{Boolean, Float, Integer, List, Map, Null, String};
        match (self, other) {
            (Null, _) | (_, Null) => None,
            (Boolean(a), Boolean(b)) => Some(a == b),
            (Integer(a), Integer(b)) => Some(a == b),
            (Float(a), Float(b)) => Some(a == b),
            // Mixed numeric: compare by value. `as f64` is exact for the i64
            // magnitudes openCypher round-trips; equality with a NaN/inf float
            // falls out of IEEE comparison correctly.
            #[allow(clippy::cast_precision_loss)]
            (Integer(a), Float(b)) => Some((*a as f64) == *b),
            #[allow(clippy::cast_precision_loss)]
            (Float(a), Integer(b)) => Some(*a == (*b as f64)),
            (String(a), String(b)) => Some(a == b),
            (List(a), List(b)) => list_cypher_equal(a, b),
            (Map(a), Map(b)) => map_cypher_equal(a, b),
            // Any other combination is a type mismatch ⇒ definitely not equal.
            _ => Some(false),
        }
    }

    /// The openCypher **orderability** total order, as used by `ORDER BY`.
    ///
    /// Unlike the `=` operator this is total and never `null`: every pair of
    /// values has a definite order so results can be sorted deterministically.
    /// The order is:
    ///
    /// 1. Values are first ordered by **type group**, in the order
    ///    number < boolean < string < list < map < null. (`null` sorts
    ///    **greatest**, so `ORDER BY ... ASC` places it last, matching Cypher.)
    /// 2. Within the numeric group, `Integer` and `Float` are ordered together
    ///    by mathematical value; `NaN` is treated as **greater than** every
    ///    other number (and equal to itself) so the relation stays total.
    /// 3. Booleans: `false` < `true`.
    /// 4. Strings: lexicographic by Unicode code point.
    /// 5. Lists: element-wise (lexicographic), shorter-is-smaller on a prefix.
    /// 6. Maps: by sorted `(key, value)` pairs, lexicographically.
    ///
    /// This relation is reflexive, antisymmetric, and transitive, so it is safe
    /// to pass to [`slice::sort_by`].
    #[must_use]
    pub fn cypher_order(&self, other: &PropertyValue) -> Ordering {
        let (ga, gb) = (self.order_group(), other.order_group());
        if ga != gb {
            return ga.cmp(&gb);
        }
        use PropertyValue::{Boolean, List, Map, String};
        match (self, other) {
            // Same numeric group: compare by value with a total NaN rule.
            _ if ga == 0 => self.numeric_order(other),
            (Boolean(a), Boolean(b)) => a.cmp(b),
            (String(a), String(b)) => a.cmp(b),
            (List(a), List(b)) => list_cypher_order(a, b),
            (Map(a), Map(b)) => map_cypher_order(a, b),
            // Both Null (group 5): equal.
            _ => Ordering::Equal,
        }
    }

    /// The type-group rank used by [`cypher_order`](Self::cypher_order).
    /// Number=0, Boolean=1, String=2, List=3, Map=4, Null=5.
    fn order_group(&self) -> u8 {
        match self {
            PropertyValue::Integer(_) | PropertyValue::Float(_) => 0,
            PropertyValue::Boolean(_) => 1,
            PropertyValue::String(_) => 2,
            PropertyValue::List(_) => 3,
            PropertyValue::Map(_) => 4,
            PropertyValue::Null => 5,
        }
    }

    /// Total order within the numeric group (Integer/Float together). `NaN`
    /// sorts greatest and is equal to itself, keeping the relation total.
    fn numeric_order(&self, other: &PropertyValue) -> Ordering {
        #[allow(clippy::cast_precision_loss)]
        fn as_f64(v: &PropertyValue) -> f64 {
            match v {
                PropertyValue::Integer(i) => *i as f64,
                PropertyValue::Float(f) => *f,
                _ => unreachable!("numeric_order called on non-numeric value"),
            }
        }
        // If both are integers, compare exactly to avoid f64 precision loss for
        // large magnitudes; otherwise fall back to the total-float order.
        if let (PropertyValue::Integer(a), PropertyValue::Integer(b)) = (self, other) {
            return a.cmp(b);
        }
        total_f64_order(as_f64(self), as_f64(other))
    }
}

/// A total order over `f64` with `NaN` sorted greatest (and equal to itself).
fn total_f64_order(a: f64, b: f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        // Neither is NaN ⇒ `partial_cmp` is always `Some`.
        (false, false) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

/// Ternary element-wise list equality for the `=` operator.
fn list_cypher_equal(a: &[PropertyValue], b: &[PropertyValue]) -> Option<bool> {
    if a.len() != b.len() {
        return Some(false);
    }
    let mut saw_unknown = false;
    for (x, y) in a.iter().zip(b.iter()) {
        match x.cypher_equal(y) {
            Some(false) => return Some(false),
            Some(true) => {}
            None => saw_unknown = true,
        }
    }
    if saw_unknown { None } else { Some(true) }
}

/// Ternary map equality for the `=` operator: unequal key sets are a definite
/// structural difference; equal key sets fold per-key value comparisons.
fn map_cypher_equal(
    a: &BTreeMap<String, PropertyValue>,
    b: &BTreeMap<String, PropertyValue>,
) -> Option<bool> {
    if a.len() != b.len() || a.keys().ne(b.keys()) {
        return Some(false);
    }
    let mut saw_unknown = false;
    for (k, x) in a {
        // Key presence guaranteed equal by the check above.
        let y = &b[k];
        match x.cypher_equal(y) {
            Some(false) => return Some(false),
            Some(true) => {}
            None => saw_unknown = true,
        }
    }
    if saw_unknown { None } else { Some(true) }
}

/// Lexicographic list orderability (shorter prefix sorts first).
fn list_cypher_order(a: &[PropertyValue], b: &[PropertyValue]) -> Ordering {
    for (x, y) in a.iter().zip(b.iter()) {
        let ord = x.cypher_order(y);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.len().cmp(&b.len())
}

/// Map orderability over sorted `(key, value)` pairs.
fn map_cypher_order(
    a: &BTreeMap<String, PropertyValue>,
    b: &BTreeMap<String, PropertyValue>,
) -> Ordering {
    let mut ai = a.iter();
    let mut bi = b.iter();
    loop {
        match (ai.next(), bi.next()) {
            (Some((ka, va)), Some((kb, vb))) => {
                let ord = ka.cmp(kb).then_with(|| va.cypher_order(vb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

/// Structural identity (the `DISTINCT` / grouping / membership notion), distinct
/// from the `=` operator. `null` is identical to `null`; values of different
/// type tags are never identical (so `Integer(1)` ≠ `Float(1.0)`); two `NaN`s
/// are identical so `DISTINCT` collapses them.
impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        use PropertyValue::{Boolean, Float, Integer, List, Map, Null, String};
        match (self, other) {
            (Null, Null) => true,
            (Boolean(a), Boolean(b)) => a == b,
            (Integer(a), Integer(b)) => a == b,
            // NaN is identical to NaN under structural identity (total_cmp
            // treats them equal); +0.0 and -0.0 are distinct bit patterns but
            // the same value — identity treats them equal.
            (Float(a), Float(b)) => a.total_cmp(b) == Ordering::Equal || a == b,
            (String(a), String(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            _ => false,
        }
    }
}

// --- Ergonomic conversions into PropertyValue -------------------------------

impl From<bool> for PropertyValue {
    fn from(v: bool) -> Self {
        PropertyValue::Boolean(v)
    }
}

impl From<i64> for PropertyValue {
    fn from(v: i64) -> Self {
        PropertyValue::Integer(v)
    }
}

impl From<i32> for PropertyValue {
    fn from(v: i32) -> Self {
        PropertyValue::Integer(i64::from(v))
    }
}

impl From<f64> for PropertyValue {
    fn from(v: f64) -> Self {
        PropertyValue::Float(v)
    }
}

impl From<String> for PropertyValue {
    fn from(v: String) -> Self {
        PropertyValue::String(v)
    }
}

impl From<&str> for PropertyValue {
    fn from(v: &str) -> Self {
        PropertyValue::String(v.to_owned())
    }
}

impl<T: Into<PropertyValue>> From<Vec<T>> for PropertyValue {
    fn from(v: Vec<T>) -> Self {
        PropertyValue::List(v.into_iter().map(Into::into).collect())
    }
}

// ---- Unit tests -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    // -- is_null / type_name --------------------------------------------------

    #[test]
    fn is_null_only_for_null() {
        assert!(PropertyValue::Null.is_null());
        assert!(!PropertyValue::Boolean(true).is_null());
        assert!(!PropertyValue::Integer(0).is_null());
        assert!(!PropertyValue::Float(0.0).is_null());
        assert!(!PropertyValue::String("".into()).is_null());
        assert!(!PropertyValue::List(vec![]).is_null());
        assert!(!PropertyValue::Map(BTreeMap::new()).is_null());
    }

    #[test]
    fn type_name_matches_variant() {
        assert_eq!(PropertyValue::Null.type_name(), "Null");
        assert_eq!(PropertyValue::Boolean(false).type_name(), "Boolean");
        assert_eq!(PropertyValue::Integer(1).type_name(), "Integer");
        assert_eq!(PropertyValue::Float(1.0).type_name(), "Float");
        assert_eq!(PropertyValue::String("x".into()).type_name(), "String");
        assert_eq!(PropertyValue::List(vec![]).type_name(), "List");
        assert_eq!(PropertyValue::Map(BTreeMap::new()).type_name(), "Map");
    }

    // -- cypher_equal ---------------------------------------------------------

    #[test]
    fn null_equality_is_always_none() {
        // null = null → None (unknown), not true
        assert_eq!(PropertyValue::Null.cypher_equal(&PropertyValue::Null), None);
        // null = 1 → None
        assert_eq!(
            PropertyValue::Null.cypher_equal(&PropertyValue::Integer(1)),
            None
        );
        // 1 = null → None
        assert_eq!(
            PropertyValue::Integer(1).cypher_equal(&PropertyValue::Null),
            None
        );
    }

    #[test]
    fn boolean_equality() {
        assert_eq!(
            PropertyValue::Boolean(true).cypher_equal(&PropertyValue::Boolean(true)),
            Some(true)
        );
        assert_eq!(
            PropertyValue::Boolean(true).cypher_equal(&PropertyValue::Boolean(false)),
            Some(false)
        );
    }

    #[test]
    fn integer_equality() {
        assert_eq!(
            PropertyValue::Integer(42).cypher_equal(&PropertyValue::Integer(42)),
            Some(true)
        );
        assert_eq!(
            PropertyValue::Integer(1).cypher_equal(&PropertyValue::Integer(2)),
            Some(false)
        );
    }

    #[test]
    fn float_equality_nan_is_not_equal() {
        // NaN = NaN → false (IEEE: NaN is not equal to anything)
        let nan = PropertyValue::Float(f64::NAN);
        assert_eq!(nan.cypher_equal(&nan), Some(false));
        assert_eq!(
            PropertyValue::Float(1.0).cypher_equal(&PropertyValue::Float(1.0)),
            Some(true)
        );
    }

    #[test]
    fn mixed_numeric_equality() {
        // 1 = 1.0 → true (mathematical value equality)
        assert_eq!(
            PropertyValue::Integer(1).cypher_equal(&PropertyValue::Float(1.0)),
            Some(true)
        );
        assert_eq!(
            PropertyValue::Float(2.0).cypher_equal(&PropertyValue::Integer(2)),
            Some(true)
        );
        // 1 = 2.0 → false
        assert_eq!(
            PropertyValue::Integer(1).cypher_equal(&PropertyValue::Float(2.0)),
            Some(false)
        );
    }

    #[test]
    fn cross_type_non_numeric_is_false() {
        // String and Integer are different types → definite false
        assert_eq!(
            PropertyValue::String("1".into()).cypher_equal(&PropertyValue::Integer(1)),
            Some(false)
        );
        assert_eq!(
            PropertyValue::Boolean(true).cypher_equal(&PropertyValue::Integer(1)),
            Some(false)
        );
    }

    #[test]
    fn list_equality_element_wise() {
        let a: PropertyValue = vec![1_i64, 2, 3].into();
        let b: PropertyValue = vec![1_i64, 2, 3].into();
        let c: PropertyValue = vec![1_i64, 2, 4].into();
        assert_eq!(a.cypher_equal(&b), Some(true));
        assert_eq!(a.cypher_equal(&c), Some(false));
    }

    #[test]
    fn list_equality_different_length_is_false() {
        let a: PropertyValue = vec![1_i64].into();
        let b: PropertyValue = vec![1_i64, 2].into();
        assert_eq!(a.cypher_equal(&b), Some(false));
    }

    #[test]
    fn list_equality_with_null_element_is_indeterminate() {
        // [1, null] = [1, null] → None (null element makes it unknown)
        let a = PropertyValue::List(vec![PropertyValue::Integer(1), PropertyValue::Null]);
        let b = PropertyValue::List(vec![PropertyValue::Integer(1), PropertyValue::Null]);
        assert_eq!(a.cypher_equal(&b), None);
    }

    #[test]
    fn list_equality_definite_inequality_wins_over_null() {
        // [1, null, 3] vs [1, null, 99]: the third element is definitely !=
        let a = PropertyValue::List(vec![
            PropertyValue::Integer(1),
            PropertyValue::Null,
            PropertyValue::Integer(3),
        ]);
        let b = PropertyValue::List(vec![
            PropertyValue::Integer(1),
            PropertyValue::Null,
            PropertyValue::Integer(99),
        ]);
        assert_eq!(a.cypher_equal(&b), Some(false));
    }

    #[test]
    fn map_equality_same_keys_and_values() {
        let mut m1 = BTreeMap::new();
        m1.insert("x".to_string(), PropertyValue::Integer(1));
        let mut m2 = BTreeMap::new();
        m2.insert("x".to_string(), PropertyValue::Integer(1));
        assert_eq!(
            PropertyValue::Map(m1).cypher_equal(&PropertyValue::Map(m2)),
            Some(true)
        );
    }

    #[test]
    fn map_equality_different_keys_is_false() {
        let mut m1 = BTreeMap::new();
        m1.insert("a".to_string(), PropertyValue::Integer(1));
        let mut m2 = BTreeMap::new();
        m2.insert("b".to_string(), PropertyValue::Integer(1));
        assert_eq!(
            PropertyValue::Map(m1).cypher_equal(&PropertyValue::Map(m2)),
            Some(false)
        );
    }

    // -- structural identity (PartialEq) --------------------------------------

    #[test]
    fn structural_identity_null_equals_null() {
        assert_eq!(PropertyValue::Null, PropertyValue::Null);
    }

    #[test]
    fn structural_identity_nan_equals_nan() {
        // Two NaN floats are structurally identical (for DISTINCT/grouping)
        assert_eq!(
            PropertyValue::Float(f64::NAN),
            PropertyValue::Float(f64::NAN)
        );
    }

    #[test]
    fn structural_identity_integer_not_equal_float() {
        // Integer(1) is NOT structurally identical to Float(1.0)
        assert_ne!(PropertyValue::Integer(1), PropertyValue::Float(1.0));
    }

    // -- cypher_order ---------------------------------------------------------

    #[test]
    fn order_type_groups_number_lt_bool_lt_string_lt_list_lt_map_lt_null() {
        use std::cmp::Ordering;
        let vals = [
            PropertyValue::Null,
            PropertyValue::Map(BTreeMap::new()),
            PropertyValue::List(vec![]),
            PropertyValue::String("a".into()),
            PropertyValue::Boolean(false),
            PropertyValue::Integer(0),
        ];
        // Every pair in order: earlier < later
        for i in 0..vals.len() {
            for j in (i + 1)..vals.len() {
                // vals is reverse of expected order (null greatest), so vals[j] < vals[i]
                assert_eq!(
                    vals[j].cypher_order(&vals[i]),
                    Ordering::Less,
                    "{} should be less than {}",
                    vals[j].type_name(),
                    vals[i].type_name()
                );
            }
        }
    }

    #[test]
    fn order_null_is_greatest() {
        use std::cmp::Ordering;
        assert_eq!(
            PropertyValue::Null.cypher_order(&PropertyValue::Integer(i64::MAX)),
            Ordering::Greater
        );
        assert_eq!(
            PropertyValue::Null.cypher_order(&PropertyValue::Null),
            Ordering::Equal
        );
    }

    #[test]
    fn order_nan_is_greatest_within_numeric() {
        use std::cmp::Ordering;
        let nan = PropertyValue::Float(f64::NAN);
        assert_eq!(
            nan.cypher_order(&PropertyValue::Float(f64::MAX)),
            Ordering::Greater
        );
        assert_eq!(nan.cypher_order(&nan), Ordering::Equal);
    }

    #[test]
    fn order_integers_numeric() {
        use std::cmp::Ordering;
        assert_eq!(
            PropertyValue::Integer(-5).cypher_order(&PropertyValue::Integer(3)),
            Ordering::Less
        );
    }

    #[test]
    fn order_mixed_numeric() {
        use std::cmp::Ordering;
        // Integer(1) and Float(1.0) are equal by value in orderability
        assert_eq!(
            PropertyValue::Integer(1).cypher_order(&PropertyValue::Float(1.0)),
            Ordering::Equal
        );
        assert_eq!(
            PropertyValue::Integer(2).cypher_order(&PropertyValue::Float(1.5)),
            Ordering::Greater
        );
    }

    #[test]
    fn order_booleans_false_lt_true() {
        use std::cmp::Ordering;
        assert_eq!(
            PropertyValue::Boolean(false).cypher_order(&PropertyValue::Boolean(true)),
            Ordering::Less
        );
    }

    #[test]
    fn order_strings_lexicographic() {
        use std::cmp::Ordering;
        assert_eq!(
            PropertyValue::String("abc".into()).cypher_order(&PropertyValue::String("abd".into())),
            Ordering::Less
        );
    }

    #[test]
    fn order_lists_elementwise() {
        use std::cmp::Ordering;
        let a: PropertyValue = vec![1_i64, 2].into();
        let b: PropertyValue = vec![1_i64, 3].into();
        assert_eq!(a.cypher_order(&b), Ordering::Less);
    }

    #[test]
    fn order_lists_prefix_shorter_is_less() {
        use std::cmp::Ordering;
        let a: PropertyValue = vec![1_i64].into();
        let b: PropertyValue = vec![1_i64, 2].into();
        assert_eq!(a.cypher_order(&b), Ordering::Less);
    }

    // -- serde round-trip -----------------------------------------------------

    #[test]
    fn serde_roundtrip_scalar() {
        let values = [
            PropertyValue::Null,
            PropertyValue::Boolean(true),
            PropertyValue::Integer(-99),
            // Use a float that doesn't approximate any well-known constant
            PropertyValue::Float(1.234_567),
            PropertyValue::String("hello".into()),
        ];
        for v in &values {
            let json = serde_json::to_string(v).expect("serialize");
            let back: PropertyValue = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*v, back, "round-trip failed for {json}");
        }
    }

    #[test]
    fn serde_roundtrip_list() {
        let v: PropertyValue = vec![1_i64, 2, 3].into();
        let json = serde_json::to_string(&v).expect("serialize");
        let back: PropertyValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn serde_roundtrip_map() {
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), PropertyValue::Integer(1));
        m.insert("b".to_string(), PropertyValue::Boolean(false));
        let v = PropertyValue::Map(m);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: PropertyValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn serde_roundtrip_nested() {
        // A list containing a map containing a null
        let mut inner = BTreeMap::new();
        inner.insert("k".to_string(), PropertyValue::Null);
        let v = PropertyValue::List(vec![PropertyValue::Map(inner), PropertyValue::Integer(42)]);
        let json = serde_json::to_string(&v).expect("serialize");
        let back: PropertyValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    // -- From conversions -----------------------------------------------------

    #[test]
    fn from_conversions() {
        assert_eq!(PropertyValue::from(true), PropertyValue::Boolean(true));
        assert_eq!(PropertyValue::from(42_i64), PropertyValue::Integer(42));
        assert_eq!(PropertyValue::from(7_i32), PropertyValue::Integer(7));
        assert_eq!(PropertyValue::from(1.5_f64), PropertyValue::Float(1.5));
        assert_eq!(
            PropertyValue::from("hi"),
            PropertyValue::String("hi".into())
        );
        assert_eq!(
            PropertyValue::from("hi".to_string()),
            PropertyValue::String("hi".into())
        );
    }
}
