//! License-clean synthetic graph dataset generator (board item T-0035).
//!
//! Benchmarks and integration tests need a representative graph with text
//! properties. Committing a third-party dataset risks a licensing violation
//! (the open-source guardrails forbid non-redistributable data), so we
//! **generate** the graph instead: a generated graph carries no external
//! licence and no PII, and is reproducible from a seed rather than downloaded.
//!
//! The two pieces:
//!
//! - [`Generator`] — streams a directed, typed, labelled property graph with a
//!   **power-law in-degree distribution** (super-nodes) of a configurable size
//!   (default 1M nodes / 10M edges), deterministic given a [`GenConfig::seed`].
//! - [`io`] — a portable JSONL form: [`write_jsonl`] serialises a generated
//!   graph to any [`std::io::Write`]; [`read_records`] reads it back. This is
//!   the format benches/tests load until the on-object storage writers
//!   (SPIKE-0003) exist; the same `Generator` can later feed those writers
//!   directly.
//!
//! # Determinism
//!
//! Everything random flows through the vendored [`SplitMix64`] PRNG, seeded from
//! [`GenConfig::seed`]. Identical config + seed ⇒ byte-identical output on every
//! platform. There is **no** dependency on the `rand` crate (whose stream is not
//! stable across versions) precisely so the committed sample stays reproducible.

mod generator;
pub mod io;
mod rng;

pub use generator::{EdgeIter, GenConfig, Generator, NodeIter};
pub use io::{GenStats, GraphRecord, read_records, write_jsonl};
pub use rng::SplitMix64;
