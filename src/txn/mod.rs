//! Transaction management: ACID guarantees over the object store.
//!
//! This module owns the single-writer / multi-reader leasing model and the
//! S3-native commit protocol. It enforces the atomicity, isolation, and
//! durability requirements from Cat. 1 (ACID) and Cat. 7 (writer leasing) of
//! the master rubric.
//!
//! **Status:** stub — the commit protocol is designed in SPIKE-0001 / SPIKE-0002
//! and implemented afterwards. This module exists so the workspace compiles
//! and the module tree is established from T-0001 onwards.

/// Placeholder marker for the not-yet-implemented transaction manager.
#[allow(dead_code)]
pub struct TransactionManager;
