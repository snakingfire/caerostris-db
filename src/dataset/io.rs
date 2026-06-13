//! Portable JSONL serialisation for generated graphs.
//!
//! Until the on-object storage writers (SPIKE-0003) land, generated graphs are
//! materialised in a **portable, line-oriented JSON** form so benches and
//! integration tests can write a graph once and load it back. Each line is one
//! self-describing JSON record:
//!
//! ```jsonl
//! {"record":"meta","node_count":1000000,"edge_count":10000000,"seed":0}
//! {"record":"node","node":{ ...Node... }}
//! {"record":"edge","edge":{ ...Edge... }}
//! ```
//!
//! JSONL is chosen deliberately: it streams (a 10M-edge file is never held in
//! memory whole, on write *or* read), it is diff-friendly for the tiny committed
//! sample, and it round-trips the logical [`model`](crate::model) types verbatim
//! through their existing serde derives — no bespoke byte format to keep in sync.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::model::{Edge, Node};

use super::Generator;

/// One line of the JSONL stream: a leading metadata record, then nodes, then
/// edges. Tagged by the `record` field so a reader can dispatch without
/// look-ahead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum GraphRecord {
    /// The header: the counts and seed the file was generated with. Always the
    /// first line, so a reader can validate size before ingesting.
    Meta {
        /// Number of node records that follow.
        node_count: u64,
        /// Number of edge records that follow.
        edge_count: u64,
        /// The seed used (records provenance for reproduction).
        seed: u64,
    },
    /// A node record.
    Node {
        /// The node.
        node: Node,
    },
    /// An edge record.
    Edge {
        /// The edge.
        edge: Edge,
    },
}

/// Summary statistics returned by [`write_jsonl`]: what was actually written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenStats {
    /// Number of node records written.
    pub nodes_written: u64,
    /// Number of edge records written.
    pub edges_written: u64,
    /// Total bytes written (including newlines and the meta line).
    pub bytes_written: u64,
}

/// Serialise a generated graph to `out` as JSONL, returning what was written.
///
/// Streams node-by-node then edge-by-edge: memory stays constant regardless of
/// graph size, so the 1M/10M default (or larger) writes without buffering the
/// whole graph. The first line is always a [`GraphRecord::Meta`] header.
///
/// # Errors
///
/// Returns the first [`io::Error`] from writing to `out` (e.g. disk full,
/// broken pipe). Serialisation of the model types is infallible.
pub fn write_jsonl<W: Write>(generator: &Generator, out: &mut W) -> io::Result<GenStats> {
    let cfg = generator.config();
    let mut bytes_written = 0u64;

    let meta = GraphRecord::Meta {
        node_count: cfg.node_count,
        edge_count: cfg.edge_count,
        seed: cfg.seed,
    };
    bytes_written += write_line(out, &meta)?;

    let mut nodes_written = 0u64;
    for node in generator.nodes() {
        bytes_written += write_line(out, &GraphRecord::Node { node })?;
        nodes_written += 1;
    }

    let mut edges_written = 0u64;
    for edge in generator.edges() {
        bytes_written += write_line(out, &GraphRecord::Edge { edge })?;
        edges_written += 1;
    }

    out.flush()?;
    Ok(GenStats {
        nodes_written,
        edges_written,
        bytes_written,
    })
}

/// Write one JSON record followed by a newline; return the bytes written.
fn write_line<W: Write>(out: &mut W, record: &GraphRecord) -> io::Result<u64> {
    // serde_json on a `String`/`Vec<u8>` is infallible for these types; map any
    // surprising error into io for a single error surface to the caller.
    let line =
        serde_json::to_string(record).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    // line length + 1 for the newline.
    Ok(line.len() as u64 + 1)
}

/// Read a JSONL graph back as a streaming iterator of records.
///
/// Each item is one parsed [`GraphRecord`]; blank lines are skipped. The
/// iterator is lazy — a multi-gigabyte file is read one line at a time, never
/// materialised whole. Use the leading [`GraphRecord::Meta`] to learn the counts.
///
/// # Errors
///
/// Each yielded `Result` carries either a parsed record or the first
/// read/parse error for that line.
pub fn read_records<R: BufRead>(reader: R) -> impl Iterator<Item = io::Result<GraphRecord>> {
    reader.lines().filter_map(|line| match line {
        Err(e) => Some(Err(e)),
        Ok(l) if l.trim().is_empty() => None,
        Ok(l) => Some(
            serde_json::from_str::<GraphRecord>(&l)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::GenConfig;
    use std::io::Cursor;

    fn make(nodes: u64, edges: u64, seed: u64) -> Generator {
        Generator::new(GenConfig::small(nodes, edges, seed))
    }

    #[test]
    fn write_then_read_round_trips_the_graph() {
        let g = make(50, 120, 9);
        let expected_nodes: Vec<Node> = g.nodes().collect();
        let expected_edges: Vec<Edge> = g.edges().collect();

        let mut buf: Vec<u8> = Vec::new();
        let stats = write_jsonl(&g, &mut buf).expect("write");
        assert_eq!(stats.nodes_written, 50);
        assert_eq!(stats.edges_written, 120);
        assert!(stats.bytes_written > 0);

        let mut got_nodes = Vec::new();
        let mut got_edges = Vec::new();
        let mut meta_seen = false;
        for rec in read_records(Cursor::new(buf)) {
            match rec.expect("parse") {
                GraphRecord::Meta {
                    node_count,
                    edge_count,
                    seed,
                } => {
                    assert_eq!((node_count, edge_count, seed), (50, 120, 9));
                    meta_seen = true;
                }
                GraphRecord::Node { node } => got_nodes.push(node),
                GraphRecord::Edge { edge } => got_edges.push(edge),
            }
        }
        assert!(meta_seen, "meta line must be present");
        assert_eq!(got_nodes, expected_nodes);
        assert_eq!(got_edges, expected_edges);
    }

    #[test]
    fn meta_is_the_first_line() {
        let g = make(3, 3, 1);
        let mut buf: Vec<u8> = Vec::new();
        write_jsonl(&g, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let first = text.lines().next().unwrap();
        assert!(
            first.contains("\"record\":\"meta\""),
            "first line was: {first}"
        );
    }

    #[test]
    fn output_is_byte_identical_for_a_fixed_seed() {
        // The committed-sample reproducibility guarantee, at the byte level.
        let mut a: Vec<u8> = Vec::new();
        let mut b: Vec<u8> = Vec::new();
        write_jsonl(&make(40, 100, 2024), &mut a).unwrap();
        write_jsonl(&make(40, 100, 2024), &mut b).unwrap();
        assert_eq!(a, b, "JSONL output is not byte-reproducible");
    }

    #[test]
    fn empty_graph_writes_only_meta() {
        let g = make(0, 0, 1);
        let mut buf: Vec<u8> = Vec::new();
        let stats = write_jsonl(&g, &mut buf).unwrap();
        assert_eq!(stats.nodes_written, 0);
        assert_eq!(stats.edges_written, 0);
        let lines: Vec<&str> = std::str::from_utf8(&buf).unwrap().lines().collect();
        assert_eq!(lines.len(), 1, "only the meta line should be present");
    }

    #[test]
    fn reader_skips_blank_lines() {
        let input = "\n{\"record\":\"meta\",\"node_count\":0,\"edge_count\":0,\"seed\":0}\n\n";
        let recs: Vec<GraphRecord> = read_records(Cursor::new(input))
            .map(|r| r.expect("parse"))
            .collect();
        assert_eq!(recs.len(), 1);
    }

    #[test]
    fn reader_reports_parse_errors() {
        let input = "{not valid json}\n";
        let mut it = read_records(Cursor::new(input));
        let first = it.next().expect("one item");
        assert!(first.is_err(), "malformed line should yield an error");
    }

    #[test]
    fn larger_graph_round_trips_bit_exact_including_float_weights() {
        // Regression guard: float weights must survive the JSON text round-trip
        // bit-for-bit, or a graph would read back unequal to the one written.
        // A 500-node / 1200-edge graph spans enough distinct weight values to
        // catch a precision regression in the writer/quantisation.
        let g = make(500, 1_200, 31);
        let expected_nodes: Vec<Node> = g.nodes().collect();
        let expected_edges: Vec<Edge> = g.edges().collect();

        let mut buf: Vec<u8> = Vec::new();
        write_jsonl(&g, &mut buf).unwrap();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for rec in read_records(Cursor::new(buf)) {
            match rec.expect("parse") {
                GraphRecord::Meta { .. } => {}
                GraphRecord::Node { node } => nodes.push(node),
                GraphRecord::Edge { edge } => edges.push(edge),
            }
        }
        assert_eq!(nodes, expected_nodes);
        assert_eq!(edges, expected_edges, "edge round-trip not bit-exact");
    }
}
