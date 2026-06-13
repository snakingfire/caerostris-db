//! Columnar node-property objects (`.ncol`) — the node side of the on-object
//! storage format.
//!
//! This module implements **ADR 0008 §2** (`docs/adr/0008-storage-format.md`):
//! nodes are stored **columnar** — partitioned by label and sorted by node id
//! into a contiguous id-band *shard*, with each property stored as an
//! independently addressable **column chunk**. A query that filters on one
//! property reads **only that column's byte range** (a single object-store
//! range-GET), never the whole node record. That is the access pattern the
//! latency selectivity-envelope theorem (ADR 0001 Part 5 #3) assumes, and the
//! land-gate **condition C3** this task must honour.
//!
//! ## What lives here (T-0007 scope)
//!
//! - [`NcolWriter`] — serialises a batch of [`Node`]s (one id-band shard of one
//!   label) into a single self-describing `.ncol` object.
//! - [`NcolReader`] — reconstructs nodes from a `.ncol` object via the
//!   [`ObjectStore`] trait, and — crucially — exposes a **columnar read**
//!   ([`NcolReader::read_column`]) that fetches only one column's chunk.
//! - [`ColumnDir`] / [`ColumnEntry`] — the column directory (the object
//!   "footer"): the per-column `(prop_key_id, logical_type, codec, chunk_off,
//!   chunk_len, …)` a reader (or the manifest partition map, §5.1) uses to
//!   address an exact byte range.
//!
//! ## What lives elsewhere (out of T-0007 scope)
//!
//! - Object **naming** and the **manifest partition map** that inlines each
//!   shard's `column_dir_off` + per-column offsets (ADR 0008 §1, §5.1) →
//!   **T-0009**.
//! - The CSR **adjacency** objects (`.adj`) and co-located projection → **T-0008**.
//! - Atomic **commit** (manifest create-only-CAS) → **T-0010**.
//!
//! The reader is written to work against *any* [`ObjectStore`] backend — the
//! in-memory [`MemoryStore`](super::MemoryStore) for unit/property tests and a
//! future S3-compatible client for integration against the mock — without
//! changing a line. The byte ranges it asks for are exactly the bytes the cost
//! model counts.
//!
//! ## On-bytes framing (per ADR 0008 §2.2)
//!
//! Little-endian, with the column directory ("footer") found by reading the last
//! [`TRAILER_LEN`] bytes (a suffix range-GET — `Range: bytes=-16` on S3/MinIO):
//!
//! ```text
//! +------------------------------------------------------+
//! | FILE HEADER (fixed, HEADER_LEN bytes)                |
//! |   magic u32 | format_version u16 | object_kind u8    |
//! |   flags u8 | id_band_lo u64 | id_band_hi u64         |
//! |   row_count u32 | column_count u16 | column_dir_off  |
//! |   u64 | content_len u64                              |
//! +------------------------------------------------------+
//! | COLUMN CHUNKS (one contiguous run per column)        |
//! +------------------------------------------------------+
//! | COLUMN DIRECTORY (at column_dir_off)                 |
//! |   per column: prop_key_id u32 | logical_type u8 |    |
//! |   codec u8 | present_bitmap_off u64 | chunk_off u64  |
//! |   | chunk_len u64 | min_digest [u8;8] | max_digest   |
//! |   [u8;8] | name_len u16 | name bytes                 |
//! +------------------------------------------------------+
//! | TRAILER (fixed, last TRAILER_LEN bytes)              |
//! |   column_dir_off u64 | blake3_prefix [u8;8]          |
//! +------------------------------------------------------+
//! ```

use std::collections::{BTreeMap, BTreeSet};

use super::{ObjectStore, StoreError};
use crate::model::{Node, NodeId, PropertyValue};

/// Object magic: `"CAE5"` + format nybble (ADR 0008 §2.2). Little-endian `u32`.
pub const NCOL_MAGIC: u32 = 0xCAE5_0001;

/// The format version this writer emits and the reader understands. A reader
/// **fails closed** on any other version (ADR 0008 §8.2; BUG-0014 lesson).
pub const NCOL_FORMAT_VERSION: u16 = 1;

/// `object_kind` discriminant for an `.ncol` object (ADR 0008 §2.2).
pub const OBJECT_KIND_NCOL: u8 = 1;

/// Reserved column name holding each row's node id (stored explicitly so a
/// sparse id band reconstructs faithfully). Begins with `':'` so it cannot
/// collide with a user property key.
pub const ID_COL: &str = ":id";

/// Reserved column name holding each node's label set (a list of strings).
pub const LABEL_COL: &str = ":labels";

/// Fixed file-header length in bytes (see module framing diagram).
///
/// `4 (magic) + 2 (version) + 1 (kind) + 1 (flags) + 8 (id_band_lo)
///  + 8 (id_band_hi) + 4 (row_count) + 2 (column_count) + 8 (column_dir_off)
///  + 8 (content_len) = 46`.
pub const HEADER_LEN: usize = 46;

/// Fixed trailer length: `8 (column_dir_off) + 8 (blake3 prefix) = 16`.
pub const TRAILER_LEN: usize = 16;

/// The on-disk codec id for a column chunk. Only the spec's floor codec
/// ([`Codec::Plain`]) is implemented in T-0007; the slot is `u8` so `dict` /
/// `delta-varint` (ADR 0008 §2.3) are forward-compatible additions (open
/// question 1). A reader **fails closed** on an unknown codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Codec {
    /// Self-describing per-value framing that round-trips every
    /// [`PropertyValue`] (the universal floor; ADR 0008 §2.3 "plain").
    Plain = 0,
}

impl Codec {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Codec::Plain),
            _ => None,
        }
    }
}

/// The logical type tag of a column, mirroring the [`PropertyValue`] variants
/// (ADR 0008 §2.2 `logical_type`). Used for diagnostics / forward-compat; the
/// `Plain` codec is self-describing so reconstruction does not depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicalType {
    /// Column whose non-null values are all the same scalar/​container kind,
    /// or `Mixed` when a column holds more than one kind (openCypher allows it).
    Mixed = 0,
    Boolean = 1,
    Integer = 2,
    Float = 3,
    String = 4,
    List = 5,
    Map = 6,
}

impl LogicalType {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => LogicalType::Boolean,
            2 => LogicalType::Integer,
            3 => LogicalType::Float,
            4 => LogicalType::String,
            5 => LogicalType::List,
            6 => LogicalType::Map,
            _ => LogicalType::Mixed,
        }
    }

    /// The logical type that describes a single present value.
    fn of_value(v: &PropertyValue) -> Self {
        match v {
            PropertyValue::Null => LogicalType::Mixed,
            PropertyValue::Boolean(_) => LogicalType::Boolean,
            PropertyValue::Integer(_) => LogicalType::Integer,
            PropertyValue::Float(_) => LogicalType::Float,
            PropertyValue::String(_) => LogicalType::String,
            PropertyValue::List(_) => LogicalType::List,
            PropertyValue::Map(_) => LogicalType::Map,
        }
    }
}

/// Errors raised while encoding or decoding an `.ncol` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NcolError {
    /// The object's bytes are too short to hold a valid header/trailer.
    Truncated {
        /// What the reader was decoding when it ran out of bytes.
        context: &'static str,
    },
    /// The leading magic number did not match [`NCOL_MAGIC`].
    BadMagic(u32),
    /// The object declares a `format_version` this reader does not understand.
    /// The reader **fails closed** rather than mis-reading bytes (ADR 0008 §8.2).
    UnsupportedVersion(u16),
    /// The object's `object_kind` is not [`OBJECT_KIND_NCOL`].
    WrongObjectKind(u8),
    /// A column chunk used a codec id this reader does not implement. Fail-closed.
    UnknownCodec(u8),
    /// The encoded bytes were internally inconsistent (a malformed length, a
    /// directory offset out of range, etc.).
    Malformed(&'static str),
    /// The requested property key is not a column in this shard.
    NoSuchColumn(String),
    /// The requested node id falls outside this shard's id band.
    IdOutOfBand {
        /// The requested id.
        id: u64,
        /// Inclusive band bounds `[lo, hi]`.
        band: (u64, u64),
    },
    /// An underlying [`ObjectStore`] operation failed.
    Store(StoreError),
}

impl std::fmt::Display for NcolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NcolError::Truncated { context } => write!(f, "truncated .ncol object while {context}"),
            NcolError::BadMagic(m) => write!(f, "bad .ncol magic: {m:#010x}"),
            NcolError::UnsupportedVersion(v) => write!(f, "unsupported .ncol format version {v}"),
            NcolError::WrongObjectKind(k) => write!(f, "object_kind {k} is not an .ncol object"),
            NcolError::UnknownCodec(c) => write!(f, "unknown column codec id {c}"),
            NcolError::Malformed(why) => write!(f, "malformed .ncol object: {why}"),
            NcolError::NoSuchColumn(k) => write!(f, "no column for property key {k:?}"),
            NcolError::IdOutOfBand { id, band } => {
                write!(f, "node id {id} outside shard id band [{}, {}]", band.0, band.1)
            }
            NcolError::Store(e) => write!(f, "object store error: {e}"),
        }
    }
}

impl std::error::Error for NcolError {}

impl From<StoreError> for NcolError {
    fn from(e: StoreError) -> Self {
        NcolError::Store(e)
    }
}

/// One column's entry in the column directory (the object footer).
///
/// These are the coordinates the manifest partition map (ADR 0008 §5.1) inlines
/// for filter-relevant columns so the planner can compute the exact byte range
/// from the manifest alone — and the coordinates [`NcolReader::read_column`]
/// uses to issue a single range-GET.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnEntry {
    /// The property-key name (the schema-catalog key; the `prop_key_id` slot in
    /// the on-bytes layout is derived from this name's catalog position by
    /// T-0009, so we carry the name itself for a self-contained shard).
    pub key: String,
    /// The logical type tag (diagnostics / forward-compat).
    pub logical_type: LogicalType,
    /// The codec used for this column chunk.
    pub codec: Codec,
    /// Byte offset of the present/absent bitmap, relative to the object start.
    pub present_bitmap_off: u64,
    /// Byte offset of the column value chunk, relative to the object start.
    pub chunk_off: u64,
    /// Byte length of the column value chunk.
    pub chunk_len: u64,
}

/// The column directory of a shard: the byte coordinates of every column.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColumnDir {
    /// Inclusive node-id band `[lo, hi]` this shard covers.
    pub id_band: (u64, u64),
    /// Number of node rows in the shard.
    pub row_count: u32,
    /// One entry per column, in deterministic (sorted-key) order.
    pub columns: Vec<ColumnEntry>,
}

impl ColumnDir {
    /// The directory entry for property `key`, if the shard has that column.
    #[must_use]
    pub fn column(&self, key: &str) -> Option<&ColumnEntry> {
        self.columns.iter().find(|c| c.key == key)
    }
}

// ---------------------------------------------------------------------------
// Little-endian byte cursor helpers (no external deps; std-only).
// ---------------------------------------------------------------------------

fn put_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn get_u16(b: &[u8], at: usize) -> Result<u16, NcolError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes(s.try_into().unwrap()))
        .ok_or(NcolError::Truncated { context: "u16" })
}
fn get_u32(b: &[u8], at: usize) -> Result<u32, NcolError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or(NcolError::Truncated { context: "u32" })
}
fn get_u64(b: &[u8], at: usize) -> Result<u64, NcolError> {
    b.get(at..at + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or(NcolError::Truncated { context: "u64" })
}

/// The `Plain` codec's self-describing per-value tag.
mod tag {
    pub const NULL: u8 = 0;
    pub const BOOL_FALSE: u8 = 1;
    pub const BOOL_TRUE: u8 = 2;
    pub const INT: u8 = 3;
    pub const FLOAT: u8 = 4;
    pub const STRING: u8 = 5;
    pub const LIST: u8 = 6;
    pub const MAP: u8 = 7;
}

/// Encode one [`PropertyValue`] with the self-describing `Plain` framing.
fn encode_value(buf: &mut Vec<u8>, v: &PropertyValue) {
    match v {
        PropertyValue::Null => buf.push(tag::NULL),
        PropertyValue::Boolean(false) => buf.push(tag::BOOL_FALSE),
        PropertyValue::Boolean(true) => buf.push(tag::BOOL_TRUE),
        PropertyValue::Integer(i) => {
            buf.push(tag::INT);
            put_u64(buf, *i as u64);
        }
        PropertyValue::Float(x) => {
            buf.push(tag::FLOAT);
            put_u64(buf, x.to_bits());
        }
        PropertyValue::String(s) => {
            buf.push(tag::STRING);
            put_u64(buf, s.len() as u64);
            buf.extend_from_slice(s.as_bytes());
        }
        PropertyValue::List(items) => {
            buf.push(tag::LIST);
            put_u64(buf, items.len() as u64);
            for it in items {
                encode_value(buf, it);
            }
        }
        PropertyValue::Map(m) => {
            buf.push(tag::MAP);
            put_u64(buf, m.len() as u64);
            for (k, val) in m {
                put_u64(buf, k.len() as u64);
                buf.extend_from_slice(k.as_bytes());
                encode_value(buf, val);
            }
        }
    }
}

/// Decode one [`PropertyValue`] from `b` starting at `*at`, advancing `*at`.
fn decode_value(b: &[u8], at: &mut usize) -> Result<PropertyValue, NcolError> {
    let t = *b.get(*at).ok_or(NcolError::Truncated { context: "value tag" })?;
    *at += 1;
    match t {
        tag::NULL => Ok(PropertyValue::Null),
        tag::BOOL_FALSE => Ok(PropertyValue::Boolean(false)),
        tag::BOOL_TRUE => Ok(PropertyValue::Boolean(true)),
        tag::INT => {
            let raw = get_u64(b, *at)?;
            *at += 8;
            Ok(PropertyValue::Integer(raw as i64))
        }
        tag::FLOAT => {
            let raw = get_u64(b, *at)?;
            *at += 8;
            Ok(PropertyValue::Float(f64::from_bits(raw)))
        }
        tag::STRING => {
            let len = get_u64(b, *at)? as usize;
            *at += 8;
            let bytes = b
                .get(*at..*at + len)
                .ok_or(NcolError::Truncated { context: "string body" })?;
            *at += len;
            let s = String::from_utf8(bytes.to_vec())
                .map_err(|_| NcolError::Malformed("non-utf8 string"))?;
            Ok(PropertyValue::String(s))
        }
        tag::LIST => {
            let n = get_u64(b, *at)? as usize;
            *at += 8;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                items.push(decode_value(b, at)?);
            }
            Ok(PropertyValue::List(items))
        }
        tag::MAP => {
            let n = get_u64(b, *at)? as usize;
            *at += 8;
            let mut m = BTreeMap::new();
            for _ in 0..n {
                let klen = get_u64(b, *at)? as usize;
                *at += 8;
                let kb = b
                    .get(*at..*at + klen)
                    .ok_or(NcolError::Truncated { context: "map key" })?;
                *at += klen;
                let k = String::from_utf8(kb.to_vec())
                    .map_err(|_| NcolError::Malformed("non-utf8 map key"))?;
                let v = decode_value(b, at)?;
                m.insert(k, v);
            }
            Ok(PropertyValue::Map(m))
        }
        other => Err(NcolError::UnknownCodec(other)),
    }
}

/// Writer for a single `.ncol` shard: a batch of nodes that share a label band,
/// sorted by node id, serialised columnar (ADR 0008 §2).
///
/// The writer is **format-only**; object naming and the manifest entry are
/// T-0009's job. It returns the serialised bytes and the [`ColumnDir`] (the
/// coordinates the manifest inlines / the reader rediscovers).
#[derive(Debug, Default)]
pub struct NcolWriter;

/// The output of [`NcolWriter::serialize`]: the object bytes plus its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedShard {
    /// The complete `.ncol` object bytes, ready to `put` at a content-addressed key.
    pub bytes: Vec<u8>,
    /// The column directory, also embedded in `bytes` (for the manifest map).
    pub dir: ColumnDir,
    /// Byte offset of the column directory within `bytes` (the manifest's
    /// `column_dir_off`).
    pub column_dir_off: u64,
}

impl NcolWriter {
    /// Serialise `nodes` (one id-band shard) into a single `.ncol` object.
    ///
    /// The nodes are sorted by id, the union of their property keys becomes the
    /// column set (in sorted order — deterministic), and each column is encoded
    /// as one contiguous chunk with a present bitmap. Labels are stored as a
    /// reserved `:label` column (a sorted list of the node's labels) so a node's
    /// full label set round-trips.
    ///
    /// # Errors
    ///
    /// Returns [`NcolError::Malformed`] if `nodes` is empty (a shard always
    /// covers a non-empty id band).
    pub fn serialize(&self, nodes: &[Node]) -> Result<SerializedShard, NcolError> {
        if nodes.is_empty() {
            return Err(NcolError::Malformed("cannot serialize an empty shard"));
        }

        // Sort by node id (the shard ordering invariant, §2.1).
        let mut rows: Vec<&Node> = nodes.iter().collect();
        rows.sort_by_key(|n| n.id.0);

        // A shard's node ids must be unique (it is a set of distinct nodes).
        for w in rows.windows(2) {
            if w[0].id == w[1].id {
                return Err(NcolError::Malformed("duplicate node id in shard"));
            }
        }

        let id_band = (rows.first().unwrap().id.0, rows.last().unwrap().id.0);
        let row_count = rows.len() as u32;

        // Column set = the reserved id + label columns + the union of property
        // keys, in deterministic sorted order. The reserved keys begin with ':'
        // and so cannot collide with a user property key (openCypher property
        // keys never begin with ':').
        let mut keys: BTreeSet<String> = BTreeSet::new();
        keys.insert(ID_COL.to_string());
        keys.insert(LABEL_COL.to_string());
        for n in &rows {
            for k in n.properties.keys() {
                keys.insert(k.clone());
            }
        }

        // Encode each column: present bitmap followed by the packed values of
        // the present rows. A row is "absent" for a property iff that node does
        // not carry the key (matching openCypher's missing-vs-null distinction:
        // a key explicitly set to Null IS present, with a Null value).
        struct EncodedColumn {
            key: String,
            logical_type: LogicalType,
            bitmap: Vec<u8>,
            values: Vec<u8>,
        }
        let mut encoded: Vec<EncodedColumn> = Vec::with_capacity(keys.len());

        let bitmap_len = row_count.div_ceil(8) as usize;
        for key in &keys {
            let mut bitmap = vec![0u8; bitmap_len];
            let mut values = Vec::new();
            let mut lt: Option<LogicalType> = None;
            for (i, n) in rows.iter().enumerate() {
                let present_value: Option<PropertyValue> = if key == ID_COL {
                    // The node id is always present. Stored explicitly so a
                    // sparse id band reconstructs correctly (ADR 0008 §2.3:
                    // the implicit dense-id optimisation is future work).
                    Some(PropertyValue::Integer(n.id.0 as i64))
                } else if key == LABEL_COL {
                    // Labels are always "present" (possibly an empty list).
                    let list = n
                        .labels
                        .iter()
                        .map(|l| PropertyValue::String(l.clone()))
                        .collect();
                    Some(PropertyValue::List(list))
                } else {
                    n.properties.get(key).cloned()
                };
                if let Some(v) = present_value {
                    bitmap[i / 8] |= 1 << (i % 8);
                    // Track a uniform logical type, or Mixed if it varies.
                    let this = LogicalType::of_value(&v);
                    lt = Some(match lt {
                        None => this,
                        Some(prev) if prev == this => prev,
                        Some(_) => LogicalType::Mixed,
                    });
                    encode_value(&mut values, &v);
                }
            }
            encoded.push(EncodedColumn {
                key: key.clone(),
                logical_type: lt.unwrap_or(LogicalType::Mixed),
                bitmap,
                values,
            });
        }

        // Lay out the object: header, then per-column [bitmap | values] chunks,
        // then the column directory, then the trailer.
        // Reserve the header; we backfill column_dir_off + content_len after.
        let mut bytes = vec![0u8; HEADER_LEN];

        let mut dir_entries: Vec<ColumnEntry> = Vec::with_capacity(encoded.len());
        for col in &encoded {
            let present_bitmap_off = bytes.len() as u64;
            bytes.extend_from_slice(&col.bitmap);
            let chunk_off = bytes.len() as u64;
            bytes.extend_from_slice(&col.values);
            let chunk_len = bytes.len() as u64 - chunk_off;
            dir_entries.push(ColumnEntry {
                key: col.key.clone(),
                logical_type: col.logical_type,
                codec: Codec::Plain,
                present_bitmap_off,
                chunk_off,
                chunk_len,
            });
        }

        let column_dir_off = bytes.len() as u64;
        // COLUMN DIRECTORY.
        put_u16(&mut bytes, dir_entries.len() as u16);
        for e in &dir_entries {
            put_u32(&mut bytes, 0); // prop_key_id: assigned by T-0009 catalog.
            bytes.push(e.logical_type as u8);
            bytes.push(e.codec as u8);
            put_u64(&mut bytes, e.present_bitmap_off);
            put_u64(&mut bytes, e.chunk_off);
            put_u64(&mut bytes, e.chunk_len);
            // min/max value digests (§2.2) — reserved; filled by T-0009 stats.
            bytes.extend_from_slice(&[0u8; 8]);
            bytes.extend_from_slice(&[0u8; 8]);
            put_u16(&mut bytes, e.key.len() as u16);
            bytes.extend_from_slice(e.key.as_bytes());
        }

        // TRAILER: duplicate column_dir_off + an object self-checksum prefix.
        put_u64(&mut bytes, column_dir_off);
        let checksum = fnv1a64(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());

        let content_len = bytes.len() as u64;

        // Backfill the header now that we know column_dir_off + content_len.
        write_header(
            &mut bytes,
            id_band,
            row_count,
            dir_entries.len() as u16,
            column_dir_off,
            content_len,
        );

        Ok(SerializedShard {
            bytes,
            dir: ColumnDir {
                id_band,
                row_count,
                columns: dir_entries,
            },
            column_dir_off,
        })
    }
}

/// Backfill the fixed file header in `bytes[0..HEADER_LEN]`.
fn write_header(
    bytes: &mut [u8],
    id_band: (u64, u64),
    row_count: u32,
    column_count: u16,
    column_dir_off: u64,
    content_len: u64,
) {
    let mut h = Vec::with_capacity(HEADER_LEN);
    put_u32(&mut h, NCOL_MAGIC);
    put_u16(&mut h, NCOL_FORMAT_VERSION);
    h.push(OBJECT_KIND_NCOL);
    h.push(0u8); // flags (bit0=checksummed could be set; reserved)
    put_u64(&mut h, id_band.0);
    put_u64(&mut h, id_band.1);
    put_u32(&mut h, row_count);
    put_u16(&mut h, column_count);
    put_u64(&mut h, column_dir_off);
    put_u64(&mut h, content_len);
    debug_assert_eq!(h.len(), HEADER_LEN);
    bytes[0..HEADER_LEN].copy_from_slice(&h);
}

/// A tiny std-only FNV-1a 64-bit checksum for the trailer self-check. ADR 0008
/// names BLAKE3 for content addressing (T-0009/commit), but that requires a
/// dependency; the **format-internal** self-check here only needs collision
/// resistance against accidental corruption, for which FNV-1a is adequate and
/// dependency-free. The content-address hash is layered above by T-0009.
fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The parsed file header of an `.ncol` object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Header {
    id_band: (u64, u64),
    row_count: u32,
    column_count: u16,
    column_dir_off: u64,
    content_len: u64,
}

fn parse_header(b: &[u8]) -> Result<Header, NcolError> {
    if b.len() < HEADER_LEN {
        return Err(NcolError::Truncated { context: "header" });
    }
    let magic = get_u32(b, 0)?;
    if magic != NCOL_MAGIC {
        return Err(NcolError::BadMagic(magic));
    }
    let version = get_u16(b, 4)?;
    if version != NCOL_FORMAT_VERSION {
        return Err(NcolError::UnsupportedVersion(version));
    }
    let kind = b[6];
    if kind != OBJECT_KIND_NCOL {
        return Err(NcolError::WrongObjectKind(kind));
    }
    // b[7] = flags (reserved).
    Ok(Header {
        id_band: (get_u64(b, 8)?, get_u64(b, 16)?),
        row_count: get_u32(b, 24)?,
        column_count: get_u16(b, 28)?,
        column_dir_off: get_u64(b, 30)?,
        content_len: get_u64(b, 38)?,
    })
}

/// Parse the column directory at `column_dir_off`, given the directory bytes.
fn parse_column_dir(
    dir_bytes: &[u8],
    id_band: (u64, u64),
    row_count: u32,
) -> Result<ColumnDir, NcolError> {
    let mut at = 0usize;
    let count = get_u16(dir_bytes, at)? as usize;
    at += 2;
    let mut columns = Vec::with_capacity(count);
    for _ in 0..count {
        let _prop_key_id = get_u32(dir_bytes, at)?;
        at += 4;
        let logical_type = LogicalType::from_u8(
            *dir_bytes
                .get(at)
                .ok_or(NcolError::Truncated { context: "logical_type" })?,
        );
        at += 1;
        let codec_raw = *dir_bytes
            .get(at)
            .ok_or(NcolError::Truncated { context: "codec" })?;
        at += 1;
        let codec = Codec::from_u8(codec_raw).ok_or(NcolError::UnknownCodec(codec_raw))?;
        let present_bitmap_off = get_u64(dir_bytes, at)?;
        at += 8;
        let chunk_off = get_u64(dir_bytes, at)?;
        at += 8;
        let chunk_len = get_u64(dir_bytes, at)?;
        at += 8;
        // Skip min/max digests (reserved here).
        at += 16;
        let name_len = get_u16(dir_bytes, at)? as usize;
        at += 2;
        let name_bytes = dir_bytes
            .get(at..at + name_len)
            .ok_or(NcolError::Truncated { context: "column name" })?;
        at += name_len;
        let key = String::from_utf8(name_bytes.to_vec())
            .map_err(|_| NcolError::Malformed("non-utf8 column name"))?;
        columns.push(ColumnEntry {
            key,
            logical_type,
            codec,
            present_bitmap_off,
            chunk_off,
            chunk_len,
        });
    }
    Ok(ColumnDir {
        id_band,
        row_count,
        columns,
    })
}

/// Reader for `.ncol` objects over an [`ObjectStore`].
///
/// All reads go through the [`ObjectStore`] trait, so the same reader serves the
/// in-memory backend (tests) and a real S3-compatible client unchanged. The
/// byte ranges it requests are exactly the bytes the latency cost model counts.
#[derive(Debug)]
pub struct NcolReader;

impl NcolReader {
    /// Discover the [`ColumnDir`] of the shard at `key` from the object's own
    /// self-describing framing (ADR 0008 §2.2, §8.3): the header gives
    /// `column_dir_off`, and the directory runs up to the trailer. This is the
    /// **recovery / tooling** path (it reads the whole object); the latency hot
    /// path instead carries the directory inline in the manifest (T-0009) and
    /// never issues a discovery read.
    ///
    /// The reader **fails closed** (a typed [`NcolError`]) on a foreign magic,
    /// an unsupported `format_version`, a wrong `object_kind`, an unknown codec,
    /// or any truncation — it never mis-reads bytes (BUG-0014 lesson).
    ///
    /// # Errors
    ///
    /// Propagates [`NcolError`] for a malformed/foreign object or a store error.
    pub fn read_dir<S: ObjectStore + ?Sized>(
        store: &S,
        key: &str,
    ) -> Result<ColumnDir, NcolError> {
        let bytes = store.get(key)?;
        let header = parse_header(&bytes)?;
        // Fail-closed: the object must be at least as long as it declares.
        if (bytes.len() as u64) < header.content_len {
            return Err(NcolError::Truncated {
                context: "object shorter than declared content_len",
            });
        }
        let dir_start = header.column_dir_off as usize;
        // The directory runs up to the trailer (the last TRAILER_LEN bytes).
        let dir_end = bytes
            .len()
            .checked_sub(TRAILER_LEN)
            .ok_or(NcolError::Truncated { context: "trailer" })?;
        if dir_end < dir_start {
            return Err(NcolError::Malformed("directory offset past trailer"));
        }
        let dir_bytes = bytes
            .get(dir_start..dir_end)
            .ok_or(NcolError::Truncated { context: "directory" })?;
        parse_column_dir(dir_bytes, header.id_band, header.row_count)
    }

    /// Columnar read (the **C3** path): fetch and decode **only** the column for
    /// property `key` — one range-GET of that column's chunk (plus its present
    /// bitmap), never the whole node record.
    ///
    /// Returns one entry per row of the shard, in id order: `Some(value)` for a
    /// present property (which may itself be [`PropertyValue::Null`]) or `None`
    /// for an absent property. Pass the [`ColumnDir`] from the manifest (hot
    /// path) or from [`read_dir`](Self::read_dir).
    ///
    /// # Errors
    ///
    /// [`NcolError::NoSuchColumn`] if the shard has no such column; otherwise a
    /// store or decode error.
    pub fn read_column<S: ObjectStore + ?Sized>(
        store: &S,
        key: &str,
        dir: &ColumnDir,
        property: &str,
    ) -> Result<Vec<Option<PropertyValue>>, NcolError> {
        let entry = dir
            .column(property)
            .ok_or_else(|| NcolError::NoSuchColumn(property.to_string()))?;
        // Fetch ONLY the present bitmap + this column's chunk — the contiguous
        // span [present_bitmap_off, chunk_off + chunk_len). This is the single
        // range-GET the cost model budgets; we never read the other columns.
        let span_start = entry.present_bitmap_off as usize;
        let span_end = (entry.chunk_off + entry.chunk_len) as usize;
        let span = store.get_range(key, span_start, span_end)?;
        let bitmap_len = (entry.chunk_off - entry.present_bitmap_off) as usize;
        let (bitmap, values) = span.split_at(bitmap_len);
        decode_column(bitmap, values, dir.row_count)
    }

    /// Reconstruct **full nodes** for the id sub-range `[lo, hi]` (inclusive) of
    /// the shard at `key`. A node-id range maps to a contiguous slice of rows
    /// (the shard is sorted by id), so this is the range-read access pattern:
    /// only the rows in range are materialised.
    ///
    /// This convenience reads every column's chunk (it reconstructs whole
    /// nodes); for a **filter** read use [`read_column`](Self::read_column),
    /// which touches one column only.
    ///
    /// # Errors
    ///
    /// Propagates [`NcolError`]; an empty result if `[lo, hi]` does not overlap
    /// the shard's id band.
    pub fn read_nodes_in_id_range<S: ObjectStore + ?Sized>(
        store: &S,
        key: &str,
        dir: &ColumnDir,
        lo: u64,
        hi: u64,
    ) -> Result<Vec<Node>, NcolError> {
        // Decode every column once (each is its own range-GET).
        let mut columns: BTreeMap<String, Vec<Option<PropertyValue>>> = BTreeMap::new();
        for c in &dir.columns {
            columns.insert(c.key.clone(), Self::read_column(store, key, dir, &c.key)?);
        }

        // The reserved id column gives each row's true node id (handles sparse
        // bands). It is always written by the writer.
        let id_col = columns
            .get(ID_COL)
            .ok_or(NcolError::Malformed("missing reserved :id column"))?;

        let mut out = Vec::new();
        for row in 0..dir.row_count as usize {
            let id = match id_col.get(row).and_then(|c| c.as_ref()) {
                Some(PropertyValue::Integer(i)) => *i as u64,
                _ => return Err(NcolError::Malformed("non-integer :id column value")),
            };
            if id < lo || id > hi {
                continue;
            }
            let mut node = Node::new(NodeId(id));
            for (col_key, col_vals) in &columns {
                if col_key == ID_COL {
                    continue;
                }
                let cell = col_vals.get(row).and_then(|c| c.as_ref());
                match (col_key.as_str(), cell) {
                    (LABEL_COL, Some(PropertyValue::List(items))) => {
                        for it in items {
                            if let PropertyValue::String(s) = it {
                                node.labels.insert(s.clone());
                            }
                        }
                    }
                    (LABEL_COL, _) => {}
                    (_, Some(v)) => {
                        node.properties.insert(col_key.clone(), v.clone());
                    }
                    (_, None) => {}
                }
            }
            out.push(node);
        }
        Ok(out)
    }

    /// Reconstruct **all** nodes in the shard at `key` (the whole id band).
    ///
    /// # Errors
    ///
    /// Propagates [`NcolError`].
    pub fn read_all<S: ObjectStore + ?Sized>(
        store: &S,
        key: &str,
        dir: &ColumnDir,
    ) -> Result<Vec<Node>, NcolError> {
        Self::read_nodes_in_id_range(store, key, dir, dir.id_band.0, dir.id_band.1)
    }
}

/// Decode one column chunk into per-row `Option<PropertyValue>` using its
/// present bitmap. Present rows pull successive values from `values`.
fn decode_column(
    bitmap: &[u8],
    values: &[u8],
    row_count: u32,
) -> Result<Vec<Option<PropertyValue>>, NcolError> {
    let mut out = Vec::with_capacity(row_count as usize);
    let mut at = 0usize;
    for row in 0..row_count as usize {
        let byte = bitmap.get(row / 8).copied().unwrap_or(0);
        let present = (byte >> (row % 8)) & 1 == 1;
        if present {
            out.push(Some(decode_value(values, &mut at)?));
        } else {
            out.push(None);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
