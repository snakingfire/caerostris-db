//! CSR adjacency objects (`.adj`) — the hop-expansion structure (ADR 0008 §3).
//!
//! This module implements the **edge side** of the on-object storage format: a
//! [`AdjacencyShardWriter`] that serialises a set of directed, typed edges for
//! one `(rel-type, direction)` group into a single source-sorted, banded
//! **Compressed-Sparse-Row** object, and an [`AdjacencyShardReader`] that, given
//! an [`ObjectStore`](super::ObjectStore) handle to that object, expands the
//! out-neighbours of any source node id via **bounded range-GETs** with a hard
//! per-expansion byte/degree cap (the §3.4 early-abort).
//!
//! # What this owns (and what it doesn't)
//!
//! T-0008 owns the on-`.adj`-bytes layout and the *single-shard* banded reader:
//!
//! - the per-object framing (header + neighbor blocks + fixed-stride offset
//!   directory + trailer), ADR 0008 §3.2;
//! - O(1) offset-directory indexing (`k = source_id − src_band_lo`), §3.2/§4;
//! - the **hard per-GET byte/degree cap** so a super-hub source can never bust
//!   the realized byte budget — the offset directory exposes `block_len` and
//!   `degree` *before* the neighbor bytes are fetched (§3.4, ADR 0008 condition
//!   C2, SPIKE-0008 F1).
//!
//! It does **not** own: the cross-shard **manifest partition map** (§5.1, which
//! maps a source id to *which* `.adj` object + `offset_dir_off`; that is
//! T-0009's job — here the reader is handed a resolved object key and reads its
//! self-describing header), the **co-located destination projection** (§3.3,
//! a planner/writer concern, T-0009 + T-0018), or the **content-address object
//! key** hash (ADR 0002 §1, T-0009).
//!
//! # On-object layout (ADR 0008 §3.2)
//!
//! ```text
//! +-------------------------------------------------------------+
//! | FILE HEADER (fixed, 52 bytes, little-endian)               |
//! |   magic           u32 = 0xCAE5_0002  (CAE5 + ADJ kind)     |
//! |   format_version  u16                                       |
//! |   object_kind     u8  = 2 (ADJ)                            |
//! |   flags           u8  (bit0 = checksummed)                 |
//! |   rel_type_id     u32                                       |
//! |   direction       u8  (0=out, 1=in)                        |
//! |   _pad            [u8;3]                                    |
//! |   src_band_lo     u64                                       |
//! |   src_band_hi     u64  (inclusive)                          |
//! |   src_count       u32  (distinct source ids in the band)   |
//! |   offset_dir_off  u64  (offset to the OFFSET DIRECTORY)    |
//! |   content_len     u64                                       |
//! +-------------------------------------------------------------+
//! | NEIGHBOR BLOCKS (one contiguous run per source id, in id   |
//! |  order; a source id with no out-edges has a zero-length    |
//! |  block)                                                     |
//! +-------------------------------------------------------------+
//! | OFFSET DIRECTORY (at offset_dir_off; fixed 16 bytes/entry, |
//! |  one entry per source id in [src_band_lo, src_band_hi])    |
//! |   block_off  u64   block_len u32   degree u32              |
//! +-------------------------------------------------------------+
//! | TRAILER (fixed, last 16 bytes)                             |
//! |   offset_dir_off u64  (duplicate, found via suffix range)  |
//! |   checksum       [u8;8] (self-checksum prefix, decision    |
//! |                          0034 — FNV-1a, not BLAKE3 yet)    |
//! +-------------------------------------------------------------+
//! ```
//!
//! A neighbour block for source id `s` is a `varint(degree)` followed by
//! `degree` entries sorted by `(target_id, edge_id)`. Each entry is a
//! zig-zag/delta-varint of `target_id` (delta against the previous entry's
//! target, so a CSR list of ascending targets is compact and repeats — a
//! multigraph — encode a `0` delta), the edge id as a varint, and the edge's
//! property map encoded with the self-describing value codec ([`value_codec`]).
//!
//! # Fail-closed
//!
//! Every decode validates `magic` + `format_version` + `object_kind` and the
//! trailer checksum and returns [`StorageFormatError`] rather than mis-reading
//! bytes (ADR 0008 §8.2; the BUG-0014 "parse must not fail open" lesson).

use std::collections::BTreeMap;

use crate::model::{Edge, EdgeId, NodeId, PropertyValue};

use super::{ObjectStore, StoreError};

/// The on-bytes format version this implementation reads and writes.
pub const FORMAT_VERSION: u16 = 1;

/// Object-family magic for an adjacency (`.adj`) shard: `"CAE5"` + kind nybble.
pub const ADJ_MAGIC: u32 = 0xCAE5_0002;

/// `object_kind` byte identifying an adjacency shard (ADR 0008 §3.2).
pub const OBJECT_KIND_ADJ: u8 = 2;

/// Fixed file-header length in bytes (see the layout diagram above; the fields
/// sum to 52 with the 3-byte pad after `direction`).
const HEADER_LEN: usize = 52;

/// Fixed offset-directory entry stride in bytes (`block_off` u64 + `block_len`
/// u32 + `degree` u32). The fixed stride is what lets the reader index the
/// directory with O(1) arithmetic (ADR 0008 §3.2).
const DIR_ENTRY_LEN: usize = 16;

/// Fixed trailer length in bytes (`offset_dir_off` u64 + checksum `[u8;8]`).
const TRAILER_LEN: usize = 16;

/// Edge-traversal direction. The writer materialises each direction as its own
/// shard so in-edge traversal is also a single banded range-GET (ADR 0008 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Out-edges: the block for source id `s` lists the targets of `(s)-[r]->(t)`.
    Out,
    /// In-edges: the block for source id `s` lists the sources of `(t)-[r]->(s)`.
    In,
}

impl Direction {
    /// The on-byte discriminant (`0 = out`, `1 = in`).
    #[must_use]
    pub fn as_byte(self) -> u8 {
        match self {
            Direction::Out => 0,
            Direction::In => 1,
        }
    }

    /// Parse the on-byte discriminant, fail-closed on any other value.
    fn from_byte(b: u8) -> Result<Self, StorageFormatError> {
        match b {
            0 => Ok(Direction::Out),
            1 => Ok(Direction::In),
            other => Err(StorageFormatError::BadDirection(other)),
        }
    }
}

/// Errors produced while encoding or decoding the `.adj` format, or while
/// reading it through an [`ObjectStore`](super::ObjectStore).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageFormatError {
    /// The object's leading bytes are not a recognised `.adj` magic.
    BadMagic {
        /// The magic value found.
        found: u32,
    },
    /// The object's `format_version` is not understood by this reader
    /// (fail-closed forward-compat; ADR 0008 §8.2).
    UnsupportedVersion {
        /// The version found in the header.
        found: u16,
    },
    /// The object's `object_kind` byte is not `ADJ`.
    WrongObjectKind {
        /// The kind byte found.
        found: u8,
    },
    /// The `direction` byte was neither `0` nor `1`.
    BadDirection(u8),
    /// The object's self-checksum did not match its bytes (corruption).
    ChecksumMismatch,
    /// The object is shorter than the framing requires, or an internal offset
    /// points outside the object.
    Truncated {
        /// Human-readable description of where the truncation was detected.
        context: String,
    },
    /// A property value used a codec tag this reader does not understand.
    BadValueTag(u8),
    /// A varint ran past the end of its buffer or overflowed 64 bits.
    BadVarint,
    /// A queried source id falls outside this shard's `[src_band_lo, src_band_hi]`.
    SourceOutOfBand {
        /// The queried source id.
        source: u64,
        /// Inclusive band low.
        band_lo: u64,
        /// Inclusive band high.
        band_hi: u64,
    },
    /// The underlying object store returned an error.
    Store(StoreError),
}

impl std::fmt::Display for StorageFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageFormatError::BadMagic { found } => {
                write!(f, "not an .adj object: bad magic {found:#010x}")
            }
            StorageFormatError::UnsupportedVersion { found } => write!(
                f,
                "unsupported .adj format_version {found} \
                 (this reader supports {FORMAT_VERSION}); refusing to parse"
            ),
            StorageFormatError::WrongObjectKind { found } => {
                write!(
                    f,
                    "wrong object_kind {found}: expected ADJ ({OBJECT_KIND_ADJ})"
                )
            }
            StorageFormatError::BadDirection(b) => write!(f, "bad direction byte {b}"),
            StorageFormatError::ChecksumMismatch => {
                write!(f, ".adj object self-checksum mismatch (corruption)")
            }
            StorageFormatError::Truncated { context } => {
                write!(f, "truncated .adj object: {context}")
            }
            StorageFormatError::BadValueTag(t) => write!(f, "unknown property value tag {t}"),
            StorageFormatError::BadVarint => write!(f, "malformed or overflowing varint"),
            StorageFormatError::SourceOutOfBand {
                source,
                band_lo,
                band_hi,
            } => write!(
                f,
                "source id {source} is outside this shard's band [{band_lo}, {band_hi}]"
            ),
            StorageFormatError::Store(e) => write!(f, "object store error: {e}"),
        }
    }
}

impl std::error::Error for StorageFormatError {}

impl From<StoreError> for StorageFormatError {
    fn from(e: StoreError) -> Self {
        StorageFormatError::Store(e)
    }
}

/// One decoded out-neighbour of a source node: the target node, the edge's id,
/// and the edge's properties. Round-trips exactly through the writer/reader.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbor {
    /// The neighbour (target for an out-edge, source for an in-edge) node id.
    pub neighbor: NodeId,
    /// The traversed edge's stable id.
    pub edge_id: EdgeId,
    /// The traversed edge's properties.
    pub properties: BTreeMap<String, PropertyValue>,
}

// ===========================================================================
// Writer
// ===========================================================================

/// Builds a single source-sorted, banded CSR `.adj` object for one
/// `(rel-type, direction)` group (ADR 0008 §3).
///
/// Add edges with [`push`](Self::push); they may arrive in any order. At
/// [`finish`](Self::finish) the writer groups edges by source id, sorts each
/// group's neighbours by `(target, edge_id)`, lays the neighbour blocks out in
/// ascending source-id order, builds the fixed-stride offset directory, and
/// emits the framed object bytes.
///
/// The shard's source-id band is `[src_band_lo, src_band_hi]`, supplied at
/// construction. Every source id in that inclusive range gets a directory entry
/// (a source with no out-edges gets a zero-length block) so the reader can index
/// the directory by pure arithmetic.
#[derive(Debug)]
pub struct AdjacencyShardWriter {
    rel_type_id: u32,
    direction: Direction,
    src_band_lo: u64,
    src_band_hi: u64,
    /// source id -> list of (neighbor, edge_id, properties)
    blocks: BTreeMap<u64, Vec<RawNeighbor>>,
}

#[derive(Debug, Clone)]
struct RawNeighbor {
    neighbor: u64,
    edge_id: u64,
    properties: BTreeMap<String, PropertyValue>,
}

impl AdjacencyShardWriter {
    /// Create a writer for the `(rel_type_id, direction)` group covering the
    /// inclusive source-id band `[src_band_lo, src_band_hi]`.
    ///
    /// # Panics
    ///
    /// Panics if `src_band_lo > src_band_hi` (an empty band is a programming
    /// error at the writer level; the partition planner never produces one).
    #[must_use]
    pub fn new(rel_type_id: u32, direction: Direction, src_band_lo: u64, src_band_hi: u64) -> Self {
        assert!(
            src_band_lo <= src_band_hi,
            "src_band_lo ({src_band_lo}) must be <= src_band_hi ({src_band_hi})"
        );
        AdjacencyShardWriter {
            rel_type_id,
            direction,
            src_band_lo,
            src_band_hi,
            blocks: BTreeMap::new(),
        }
    }

    /// Record one directed edge in this shard.
    ///
    /// For [`Direction::Out`] the block key is the edge's `source` and the
    /// neighbour is its `target`; for [`Direction::In`] the roles swap, so an
    /// in-shard indexes targets to their in-neighbours.
    ///
    /// # Panics
    ///
    /// Panics if the resolved source id falls outside this shard's band — that
    /// is a partitioning bug (the caller routed an edge to the wrong shard).
    pub fn push(&mut self, edge: &Edge) {
        let (src, nbr) = match self.direction {
            Direction::Out => (edge.source.get(), edge.target.get()),
            Direction::In => (edge.target.get(), edge.source.get()),
        };
        assert!(
            src >= self.src_band_lo && src <= self.src_band_hi,
            "edge source id {src} is outside shard band [{}, {}]",
            self.src_band_lo,
            self.src_band_hi
        );
        self.blocks.entry(src).or_default().push(RawNeighbor {
            neighbor: nbr,
            edge_id: edge.id.get(),
            properties: edge.properties.clone(),
        });
    }

    /// The number of distinct source ids that currently have at least one edge.
    #[must_use]
    pub fn distinct_sources(&self) -> usize {
        self.blocks.len()
    }

    /// Serialise the shard into a framed `.adj` object's bytes.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        // Sort each source's neighbours by (neighbor, edge_id). `edge_id` is
        // unique per edge, so this is a total order even in a multigraph and
        // makes the on-bytes layout deterministic (round-trip stability).
        for nbrs in self.blocks.values_mut() {
            nbrs.sort_by_key(|n| (n.neighbor, n.edge_id));
        }

        let band_width = (self.src_band_hi - self.src_band_lo + 1) as usize;

        // ---- Neighbour blocks (in ascending source-id order). ----
        let mut block_section: Vec<u8> = Vec::new();
        // Per-source directory entry, indexed by k = src - src_band_lo.
        let mut dir: Vec<(u64, u32, u32)> = vec![(0, 0, 0); band_width];

        for (&src, nbrs) in &self.blocks {
            let k = (src - self.src_band_lo) as usize;
            let block_off = (HEADER_LEN + block_section.len()) as u64;
            let mut block: Vec<u8> = Vec::new();
            write_varint(&mut block, nbrs.len() as u64);
            let mut prev_target: u64 = 0;
            for nbr in nbrs {
                // Delta against previous target (ascending; repeats => 0).
                let delta = nbr.neighbor.wrapping_sub(prev_target);
                write_varint(&mut block, delta);
                prev_target = nbr.neighbor;
                write_varint(&mut block, nbr.edge_id);
                encode_properties(&mut block, &nbr.properties);
            }
            let block_len = block.len() as u32;
            let degree = nbrs.len() as u32;
            dir[k] = (block_off, block_len, degree);
            block_section.extend_from_slice(&block);
        }

        // ---- Offset directory. ----
        let offset_dir_off = (HEADER_LEN + block_section.len()) as u64;
        let mut dir_section: Vec<u8> = Vec::with_capacity(band_width * DIR_ENTRY_LEN);
        for (block_off, block_len, degree) in &dir {
            dir_section.extend_from_slice(&block_off.to_le_bytes());
            dir_section.extend_from_slice(&block_len.to_le_bytes());
            dir_section.extend_from_slice(&degree.to_le_bytes());
        }

        let content_len =
            (HEADER_LEN + block_section.len() + dir_section.len() + TRAILER_LEN) as u64;

        // ---- Header. ----
        let mut out: Vec<u8> = Vec::with_capacity(content_len as usize);
        out.extend_from_slice(&ADJ_MAGIC.to_le_bytes()); // 4
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes()); // 2
        out.push(OBJECT_KIND_ADJ); // 1
        out.push(0x01); // flags: bit0 = checksummed
        out.extend_from_slice(&self.rel_type_id.to_le_bytes()); // 4
        out.push(self.direction.as_byte()); // 1
        out.extend_from_slice(&[0u8; 3]); // pad -> 8-byte align
        out.extend_from_slice(&self.src_band_lo.to_le_bytes()); // 8
        out.extend_from_slice(&self.src_band_hi.to_le_bytes()); // 8
        out.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes()); // 4
        out.extend_from_slice(&offset_dir_off.to_le_bytes()); // 8
        out.extend_from_slice(&content_len.to_le_bytes()); // 8
        debug_assert_eq!(out.len(), HEADER_LEN);

        // ---- Body + directory. ----
        out.extend_from_slice(&block_section);
        out.extend_from_slice(&dir_section);

        // ---- Trailer: offset_dir_off duplicate + self-checksum over all
        //      preceding bytes (decision 0034). ----
        out.extend_from_slice(&offset_dir_off.to_le_bytes());
        let checksum = fnv1a64(&out).to_le_bytes();
        out.extend_from_slice(&checksum);
        debug_assert_eq!(out.len() as u64, content_len);

        out
    }
}

// ===========================================================================
// Reader
// ===========================================================================

/// A read handle over a single `.adj` object held in an
/// [`ObjectStore`](super::ObjectStore).
///
/// Construction reads and validates the fixed header (and verifies the
/// self-checksum once, eagerly, so a corrupt object fails closed before any
/// hop). Neighbour expansion then reads only the offset-directory slice for the
/// queried source and the neighbour block itself, via bounded range-GETs — never
/// the whole object — and enforces the §3.4 hard per-expansion byte/degree cap.
#[derive(Debug, Clone)]
pub struct AdjacencyShardReader<S> {
    store: S,
    key: String,
    rel_type_id: u32,
    direction: Direction,
    src_band_lo: u64,
    src_band_hi: u64,
    offset_dir_off: u64,
    content_len: u64,
}

/// The outcome of a bounded neighbour expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct Expansion {
    /// The neighbours read (possibly truncated by the cap).
    pub neighbors: Vec<Neighbor>,
    /// The number of object-store `get_range` calls this expansion issued.
    /// Asserted in tests to prove the bounded-GET / `r ≤ 1` access pattern.
    pub gets: usize,
    /// The number of bytes actually fetched from the store (header excluded —
    /// the header is read once at open). Stays within the cap.
    pub bytes_read: usize,
    /// `true` if the cap stopped the expansion before reading the whole block —
    /// i.e. the read was early-aborted (§3.4).
    pub truncated: bool,
}

/// A hard cap on a single neighbour expansion (ADR 0008 §3.4 / SPIKE-0008 F1).
///
/// The reader consults the offset directory's `block_len`/`degree` *before*
/// fetching neighbour bytes; if a block would exceed `max_bytes`, the read is
/// truncated so a super-hub source can never bust the realized byte budget.
/// `max_neighbors` caps the count (LIMIT-driven early termination).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandCap {
    /// Maximum bytes to fetch for the neighbour block. The reader requests at
    /// most this many bytes from the block.
    pub max_bytes: usize,
    /// Maximum neighbours to return (LIMIT). `usize::MAX` for "no count cap".
    pub max_neighbors: usize,
}

impl ExpandCap {
    /// A cap that never truncates — read the whole block, all neighbours.
    #[must_use]
    pub fn unbounded() -> Self {
        ExpandCap {
            max_bytes: usize::MAX,
            max_neighbors: usize::MAX,
        }
    }

    /// A cap of `max_bytes` bytes with no count limit.
    #[must_use]
    pub fn bytes(max_bytes: usize) -> Self {
        ExpandCap {
            max_bytes,
            max_neighbors: usize::MAX,
        }
    }
}

impl<S: ObjectStore> AdjacencyShardReader<S> {
    /// Open a reader over the `.adj` object at `key`, reading and validating its
    /// header.
    ///
    /// In the full engine the `key`, `offset_dir_off`, and band come from the
    /// manifest partition map (ADR 0008 §5.1) so no discovery GET is needed on
    /// the hot path; here, given only the key, the reader self-describes from
    /// the header + trailer (§8.3). The whole object is read **once** at open to
    /// verify the self-checksum, then cached header fields drive bounded reads.
    ///
    /// # Errors
    ///
    /// Returns [`StorageFormatError`] if the object is missing, truncated,
    /// carries the wrong magic/version/kind, or fails its self-checksum.
    pub fn open(store: S, key: &str) -> Result<Self, StorageFormatError> {
        let bytes = store.get(key)?;
        let header = AdjHeader::parse(&bytes)?;
        verify_checksum(&bytes)?;
        Ok(AdjacencyShardReader {
            store,
            key: key.to_owned(),
            rel_type_id: header.rel_type_id,
            direction: header.direction,
            src_band_lo: header.src_band_lo,
            src_band_hi: header.src_band_hi,
            offset_dir_off: header.offset_dir_off,
            content_len: header.content_len,
        })
    }

    /// The relationship-type id this shard stores.
    #[must_use]
    pub fn rel_type_id(&self) -> u32 {
        self.rel_type_id
    }

    /// The edge direction this shard stores.
    #[must_use]
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// The inclusive source-id band this shard covers.
    #[must_use]
    pub fn src_band(&self) -> (u64, u64) {
        (self.src_band_lo, self.src_band_hi)
    }

    /// Read the offset-directory entry `(block_off, block_len, degree)` for a
    /// source id via a single 16-byte range-GET — the O(1) directory probe.
    ///
    /// # Errors
    ///
    /// [`StorageFormatError::SourceOutOfBand`] if the id is not in this shard's
    /// band; [`StorageFormatError::Store`] / [`StorageFormatError::Truncated`]
    /// on a backend or framing fault.
    fn read_dir_entry(&self, source: u64) -> Result<(u64, u32, u32, usize), StorageFormatError> {
        if source < self.src_band_lo || source > self.src_band_hi {
            return Err(StorageFormatError::SourceOutOfBand {
                source,
                band_lo: self.src_band_lo,
                band_hi: self.src_band_hi,
            });
        }
        let k = (source - self.src_band_lo) as usize;
        let entry_off = self.offset_dir_off as usize + k * DIR_ENTRY_LEN;
        let entry = self
            .store
            .get_range(&self.key, entry_off, entry_off + DIR_ENTRY_LEN)?;
        if entry.len() != DIR_ENTRY_LEN {
            return Err(StorageFormatError::Truncated {
                context: format!("offset directory entry for source {source}"),
            });
        }
        let block_off = u64::from_le_bytes(entry[0..8].try_into().unwrap());
        let block_len = u32::from_le_bytes(entry[8..12].try_into().unwrap());
        let degree = u32::from_le_bytes(entry[12..16].try_into().unwrap());
        Ok((block_off, block_len, degree, DIR_ENTRY_LEN))
    }

    /// The out-degree of `source` from the directory alone (no neighbour-byte
    /// fetch). One range-GET. Used by the planner / early-abort to size a read.
    ///
    /// # Errors
    ///
    /// As [`read_dir_entry`](Self::read_dir_entry).
    pub fn degree(&self, source: u64) -> Result<u32, StorageFormatError> {
        let (_, _, degree, _) = self.read_dir_entry(source)?;
        Ok(degree)
    }

    /// Expand the out-neighbours of `source`, capped by `cap` (§3.4).
    ///
    /// Issues one range-GET for the directory entry and (if the source has any
    /// edges and the cap permits) one bounded range-GET for the neighbour block,
    /// truncating the requested byte length at `cap.max_bytes`. Decoding stops
    /// once `cap.max_neighbors` neighbours are produced or the fetched bytes are
    /// exhausted. A super-hub source whose `block_len` exceeds the cap therefore
    /// never causes a read beyond the cap — the read is hard-capped from the
    /// directory `block_len` *before* the bytes are fetched.
    ///
    /// # Errors
    ///
    /// Propagates directory/store/decoding faults as [`StorageFormatError`].
    pub fn expand(&self, source: u64, cap: ExpandCap) -> Result<Expansion, StorageFormatError> {
        let (block_off, block_len, degree, dir_bytes) = self.read_dir_entry(source)?;
        let mut gets = 1usize;
        let mut bytes_read = dir_bytes;

        if degree == 0 || block_len == 0 || cap.max_neighbors == 0 {
            return Ok(Expansion {
                neighbors: Vec::new(),
                gets,
                bytes_read,
                truncated: false,
            });
        }

        // Hard per-GET byte cap: request at most `cap.max_bytes` of the block.
        let block_len = block_len as usize;
        let want = block_len.min(cap.max_bytes);
        let mut truncated = want < block_len;

        let start = block_off as usize;
        let end = start + want;
        let buf = self.store.get_range(&self.key, start, end)?;
        gets += 1;
        bytes_read += buf.len();

        // The buffer is `capped` when we deliberately fetched fewer bytes than
        // the full block (`want < block_len`). On a capped buffer, a varint that
        // runs off the end — including the *leading degree* varint when the cap
        // is 0 or below the degree's encoded width — is the §3.4 early-abort, not
        // corruption. On a full buffer, the checksum (validated at `open()`)
        // guards integrity, so any decode failure there still fails closed.
        let (neighbors, decode_truncated) =
            decode_block_prefix(&buf, degree as usize, cap.max_neighbors, truncated)?;
        truncated |= decode_truncated;

        Ok(Expansion {
            neighbors,
            gets,
            bytes_read,
            truncated,
        })
    }

    /// Expand all out-neighbours of `source` with no cap (round-trip / tests).
    ///
    /// # Errors
    ///
    /// As [`expand`](Self::expand).
    pub fn neighbors(&self, source: u64) -> Result<Vec<Neighbor>, StorageFormatError> {
        Ok(self.expand(source, ExpandCap::unbounded())?.neighbors)
    }

    /// The total object length in bytes (from the header `content_len`).
    #[must_use]
    pub fn content_len(&self) -> u64 {
        self.content_len
    }
}

// ===========================================================================
// Header parsing + checksum
// ===========================================================================

#[derive(Debug)]
struct AdjHeader {
    rel_type_id: u32,
    direction: Direction,
    src_band_lo: u64,
    src_band_hi: u64,
    offset_dir_off: u64,
    content_len: u64,
}

impl AdjHeader {
    fn parse(bytes: &[u8]) -> Result<Self, StorageFormatError> {
        if bytes.len() < HEADER_LEN + TRAILER_LEN {
            return Err(StorageFormatError::Truncated {
                context: "object shorter than header + trailer".to_owned(),
            });
        }
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != ADJ_MAGIC {
            return Err(StorageFormatError::BadMagic { found: magic });
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(StorageFormatError::UnsupportedVersion { found: version });
        }
        let kind = bytes[6];
        if kind != OBJECT_KIND_ADJ {
            return Err(StorageFormatError::WrongObjectKind { found: kind });
        }
        // bytes[7] = flags (bit0 = checksummed); validated implicitly by checksum.
        let rel_type_id = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let direction = Direction::from_byte(bytes[12])?;
        // bytes[13..16] = pad
        let src_band_lo = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let src_band_hi = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        // bytes[32..36] = src_count (advisory; directory band width is canonical)
        let offset_dir_off = u64::from_le_bytes(bytes[36..44].try_into().unwrap());
        let content_len = u64::from_le_bytes(bytes[44..52].try_into().unwrap());

        if src_band_lo > src_band_hi {
            return Err(StorageFormatError::Truncated {
                context: "header src_band_lo > src_band_hi".to_owned(),
            });
        }
        if content_len as usize != bytes.len() {
            return Err(StorageFormatError::Truncated {
                context: format!(
                    "header content_len {content_len} != actual object length {}",
                    bytes.len()
                ),
            });
        }
        // Directory must fit: offset_dir_off + band_width*stride + trailer == len.
        let band_width = (src_band_hi - src_band_lo + 1) as usize;
        let dir_end = offset_dir_off as usize + band_width * DIR_ENTRY_LEN;
        if dir_end + TRAILER_LEN != bytes.len() {
            return Err(StorageFormatError::Truncated {
                context: "offset directory does not span to the trailer".to_owned(),
            });
        }
        Ok(AdjHeader {
            rel_type_id,
            direction,
            src_band_lo,
            src_band_hi,
            offset_dir_off,
            content_len,
        })
    }
}

/// Verify the trailer self-checksum over all preceding bytes (fail-closed).
fn verify_checksum(bytes: &[u8]) -> Result<(), StorageFormatError> {
    if bytes.len() < TRAILER_LEN {
        return Err(StorageFormatError::Truncated {
            context: "object shorter than trailer".to_owned(),
        });
    }
    let split = bytes.len() - 8;
    let expected = u64::from_le_bytes(bytes[split..].try_into().unwrap());
    let actual = fnv1a64(&bytes[..split]);
    if expected != actual {
        return Err(StorageFormatError::ChecksumMismatch);
    }
    Ok(())
}

// ===========================================================================
// Neighbour-block decoding
// ===========================================================================

/// Decode up to `max_neighbors` neighbours from a (possibly truncated) block
/// buffer that should contain `degree` entries. Returns the decoded neighbours
/// and whether decoding stopped early (cap hit or buffer exhausted mid-entry).
///
/// `buffer_capped` is `true` when the caller deliberately fetched fewer bytes
/// than the full block (a §3.4 / C2 byte-budget early-abort). On a capped
/// buffer, the byte range can end *before or inside* the leading `degree`
/// varint (e.g. cap = 0, or cap = 1 with degree >= 128): that is a clean
/// early-abort yielding zero neighbours, not corruption. On a non-capped (full)
/// buffer the leading varint must decode — a failure there is genuine
/// corruption and fails closed (the block checksum is validated at `open()`).
fn decode_block_prefix(
    buf: &[u8],
    degree: usize,
    max_neighbors: usize,
    buffer_capped: bool,
) -> Result<(Vec<Neighbor>, bool), StorageFormatError> {
    let mut cursor = Cursor::new(buf);
    // The block leads with its own degree varint; trust the directory `degree`
    // as the canonical count but validate consistency when the prefix is whole.
    let encoded_degree = match cursor.read_varint() {
        Ok(d) => d as usize,
        // Ran off the *capped* buffer before the degree varint completed: this
        // is the budget-driven early-abort (cap below the degree prefix), not
        // corruption. Return an empty, truncated prefix.
        Err(StorageFormatError::BadVarint | StorageFormatError::Truncated { .. })
            if buffer_capped =>
        {
            return Ok((Vec::new(), true));
        }
        Err(e) => return Err(e),
    };
    let want = degree.min(max_neighbors);
    let mut neighbors = Vec::with_capacity(want.min(encoded_degree));
    let mut prev_target: u64 = 0;
    let mut truncated = false;

    for i in 0..encoded_degree {
        if neighbors.len() >= max_neighbors {
            truncated = true;
            break;
        }
        // A truncated byte range may cut an entry short: stop cleanly.
        match decode_one_neighbor(&mut cursor, &mut prev_target) {
            Ok(n) => neighbors.push(n),
            Err(StorageFormatError::BadVarint) | Err(StorageFormatError::Truncated { .. }) => {
                // Ran off the (capped) buffer mid-entry — this is the early
                // abort, not corruption: we deliberately fetched fewer bytes.
                truncated = true;
                break;
            }
            Err(e) => return Err(e),
        }
        let _ = i;
    }

    // If we consumed the whole intended block (not capped) the encoded degree
    // must match the directory degree — a consistency guard against corruption.
    if !truncated && max_neighbors >= degree && encoded_degree != degree {
        return Err(StorageFormatError::Truncated {
            context: format!(
                "block degree {encoded_degree} disagrees with directory degree {degree}"
            ),
        });
    }
    Ok((neighbors, truncated))
}

fn decode_one_neighbor(
    cursor: &mut Cursor<'_>,
    prev_target: &mut u64,
) -> Result<Neighbor, StorageFormatError> {
    let delta = cursor.read_varint()?;
    let target = prev_target.wrapping_add(delta);
    *prev_target = target;
    let edge_id = cursor.read_varint()?;
    let properties = decode_properties(cursor)?;
    Ok(Neighbor {
        neighbor: NodeId(target),
        edge_id: EdgeId(edge_id),
        properties,
    })
}

// ===========================================================================
// Self-describing property value codec
// ===========================================================================

mod value_codec {
    //! Tags for the self-describing property-value encoding.
    pub const NULL: u8 = 0;
    pub const BOOL_FALSE: u8 = 1;
    pub const BOOL_TRUE: u8 = 2;
    pub const INT: u8 = 3;
    pub const FLOAT: u8 = 4;
    pub const STRING: u8 = 5;
    pub const LIST: u8 = 6;
    pub const MAP: u8 = 7;
}

/// Encode a property map: `varint(len)` then `len` × (`varint(key_len)` key bytes
/// + value). Keys come from a [`BTreeMap`] so order is deterministic.
fn encode_properties(out: &mut Vec<u8>, props: &BTreeMap<String, PropertyValue>) {
    write_varint(out, props.len() as u64);
    for (k, v) in props {
        write_varint(out, k.len() as u64);
        out.extend_from_slice(k.as_bytes());
        encode_value(out, v);
    }
}

fn encode_value(out: &mut Vec<u8>, v: &PropertyValue) {
    match v {
        PropertyValue::Null => out.push(value_codec::NULL),
        PropertyValue::Boolean(false) => out.push(value_codec::BOOL_FALSE),
        PropertyValue::Boolean(true) => out.push(value_codec::BOOL_TRUE),
        PropertyValue::Integer(i) => {
            out.push(value_codec::INT);
            write_varint(out, zigzag_encode(*i));
        }
        PropertyValue::Float(f) => {
            out.push(value_codec::FLOAT);
            out.extend_from_slice(&f.to_le_bytes());
        }
        PropertyValue::String(s) => {
            out.push(value_codec::STRING);
            write_varint(out, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        PropertyValue::List(items) => {
            out.push(value_codec::LIST);
            write_varint(out, items.len() as u64);
            for item in items {
                encode_value(out, item);
            }
        }
        PropertyValue::Map(m) => {
            out.push(value_codec::MAP);
            write_varint(out, m.len() as u64);
            for (k, val) in m {
                write_varint(out, k.len() as u64);
                out.extend_from_slice(k.as_bytes());
                encode_value(out, val);
            }
        }
    }
}

fn decode_properties(
    cursor: &mut Cursor<'_>,
) -> Result<BTreeMap<String, PropertyValue>, StorageFormatError> {
    let n = cursor.read_varint()? as usize;
    let mut map = BTreeMap::new();
    for _ in 0..n {
        let key = cursor.read_string()?;
        let value = decode_value(cursor)?;
        map.insert(key, value);
    }
    Ok(map)
}

fn decode_value(cursor: &mut Cursor<'_>) -> Result<PropertyValue, StorageFormatError> {
    let tag = cursor.read_u8()?;
    match tag {
        value_codec::NULL => Ok(PropertyValue::Null),
        value_codec::BOOL_FALSE => Ok(PropertyValue::Boolean(false)),
        value_codec::BOOL_TRUE => Ok(PropertyValue::Boolean(true)),
        value_codec::INT => Ok(PropertyValue::Integer(zigzag_decode(cursor.read_varint()?))),
        value_codec::FLOAT => Ok(PropertyValue::Float(f64::from_le_bytes(
            cursor.read_array8()?,
        ))),
        value_codec::STRING => Ok(PropertyValue::String(cursor.read_string()?)),
        value_codec::LIST => {
            let n = cursor.read_varint()? as usize;
            let mut items = Vec::with_capacity(n.min(1024));
            for _ in 0..n {
                items.push(decode_value(cursor)?);
            }
            Ok(PropertyValue::List(items))
        }
        value_codec::MAP => {
            let n = cursor.read_varint()? as usize;
            let mut map = BTreeMap::new();
            for _ in 0..n {
                let key = cursor.read_string()?;
                let value = decode_value(cursor)?;
                map.insert(key, value);
            }
            Ok(PropertyValue::Map(map))
        }
        other => Err(StorageFormatError::BadValueTag(other)),
    }
}

// ===========================================================================
// Byte primitives: LEB128 varint, zig-zag, FNV-1a, a bounds-checked cursor
// ===========================================================================

/// Append an unsigned LEB128 varint.
fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

/// Zig-zag encode a signed integer into an unsigned one (small magnitudes →
/// small varints regardless of sign).
fn zigzag_encode(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn zigzag_decode(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// FNV-1a 64-bit hash — a small, dependency-free integrity checksum for the
/// object trailer (decision 0034). Not a cryptographic hash and not the
/// content-address key (that BLAKE3 keying is ADR 0002 / T-0009).
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// A bounds-checked forward cursor over a byte slice. Every read returns
/// [`StorageFormatError`] rather than panicking, so a truncated (capped) or
/// corrupt buffer fails closed.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, StorageFormatError> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or(StorageFormatError::Truncated {
                context: "expected one more byte".to_owned(),
            })?;
        self.pos += 1;
        Ok(b)
    }

    fn read_varint(&mut self) -> Result<u64, StorageFormatError> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if shift >= 64 {
                return Err(StorageFormatError::BadVarint);
            }
            let byte = *self
                .bytes
                .get(self.pos)
                .ok_or(StorageFormatError::BadVarint)?;
            self.pos += 1;
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], StorageFormatError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(StorageFormatError::BadVarint)?;
        if end > self.bytes.len() {
            return Err(StorageFormatError::Truncated {
                context: format!("expected {n} more bytes"),
            });
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn read_string(&mut self) -> Result<String, StorageFormatError> {
        let n = self.read_varint()? as usize;
        let slice = self.read_bytes(n)?;
        String::from_utf8(slice.to_vec()).map_err(|_| StorageFormatError::Truncated {
            context: "invalid UTF-8 in string property".to_owned(),
        })
    }

    fn read_array8(&mut self) -> Result<[u8; 8], StorageFormatError> {
        let slice = self.read_bytes(8)?;
        Ok(slice.try_into().unwrap())
    }
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Edge;
    use crate::storage::MemoryStore;

    const KEY: &str = "db/data/hash/adj-FOLLOWS-out-0.adj";

    /// Build, store, and open a shard for `[lo, hi]` over the given edges.
    fn round_trip(
        edges: &[Edge],
        dir: Direction,
        lo: u64,
        hi: u64,
    ) -> AdjacencyShardReader<MemoryStore> {
        let mut w = AdjacencyShardWriter::new(7, dir, lo, hi);
        for e in edges {
            w.push(e);
        }
        let bytes = w.finish();
        let mut store = MemoryStore::new();
        store.put(KEY, bytes).unwrap();
        AdjacencyShardReader::open(store, KEY).unwrap()
    }

    #[test]
    fn empty_shard_round_trips_with_zero_blocks() {
        let reader = round_trip(&[], Direction::Out, 0, 3);
        assert_eq!(reader.src_band(), (0, 3));
        assert_eq!(reader.direction(), Direction::Out);
        assert_eq!(reader.rel_type_id(), 7);
        for s in 0..=3 {
            assert_eq!(reader.degree(s).unwrap(), 0);
            assert!(reader.neighbors(s).unwrap().is_empty());
        }
    }

    #[test]
    fn single_edge_round_trips_exactly() {
        let e = Edge::new(1_u64, "FOLLOWS", 5_u64, 9_u64).with_property("since", 2020_i64);
        let reader = round_trip(&[e], Direction::Out, 5, 5);
        let nbrs = reader.neighbors(5).unwrap();
        assert_eq!(nbrs.len(), 1);
        assert_eq!(nbrs[0].neighbor, NodeId(9));
        assert_eq!(nbrs[0].edge_id, EdgeId(1));
        assert_eq!(
            nbrs[0].properties.get("since"),
            Some(&PropertyValue::Integer(2020))
        );
    }

    #[test]
    fn multiple_neighbors_sorted_and_complete() {
        let edges = vec![
            Edge::new(1_u64, "FOLLOWS", 10_u64, 30_u64),
            Edge::new(2_u64, "FOLLOWS", 10_u64, 20_u64),
            Edge::new(3_u64, "FOLLOWS", 10_u64, 25_u64),
        ];
        let reader = round_trip(&edges, Direction::Out, 10, 10);
        assert_eq!(reader.degree(10).unwrap(), 3);
        let nbrs = reader.neighbors(10).unwrap();
        let ids: Vec<u64> = nbrs.iter().map(|n| n.neighbor.get()).collect();
        assert_eq!(ids, vec![20, 25, 30], "neighbours must be target-sorted");
    }

    #[test]
    fn multigraph_parallel_edges_distinguished_by_edge_id() {
        // Two edges between the same (src, dst) with different ids + props.
        let edges = vec![
            Edge::new(100_u64, "RATED", 1_u64, 2_u64).with_property("score", 4_i64),
            Edge::new(101_u64, "RATED", 1_u64, 2_u64).with_property("score", 5_i64),
        ];
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        let nbrs = reader.neighbors(1).unwrap();
        assert_eq!(nbrs.len(), 2);
        assert_eq!(nbrs[0].edge_id, EdgeId(100));
        assert_eq!(nbrs[1].edge_id, EdgeId(101));
        assert_eq!(
            nbrs[0].properties.get("score"),
            Some(&PropertyValue::Integer(4))
        );
        assert_eq!(
            nbrs[1].properties.get("score"),
            Some(&PropertyValue::Integer(5))
        );
    }

    #[test]
    fn in_direction_indexes_targets_to_sources() {
        let edges = vec![
            Edge::new(1_u64, "FOLLOWS", 5_u64, 42_u64),
            Edge::new(2_u64, "FOLLOWS", 7_u64, 42_u64),
        ];
        // In-shard: block key is the target (42); neighbours are the sources.
        let reader = round_trip(&edges, Direction::In, 42, 42);
        let nbrs = reader.neighbors(42).unwrap();
        let ids: Vec<u64> = nbrs.iter().map(|n| n.neighbor.get()).collect();
        assert_eq!(ids, vec![5, 7]);
    }

    #[test]
    fn band_with_gaps_zero_length_blocks() {
        // Only source 2 has edges; 0,1,3,4 are empty but addressable.
        let edges = vec![Edge::new(1_u64, "FOLLOWS", 2_u64, 9_u64)];
        let reader = round_trip(&edges, Direction::Out, 0, 4);
        assert_eq!(reader.degree(0).unwrap(), 0);
        assert_eq!(reader.degree(1).unwrap(), 0);
        assert_eq!(reader.degree(2).unwrap(), 1);
        assert_eq!(reader.degree(3).unwrap(), 0);
        assert_eq!(reader.neighbors(2).unwrap()[0].neighbor, NodeId(9));
    }

    #[test]
    fn all_property_value_kinds_round_trip() {
        let mut e = Edge::new(1_u64, "T", 0_u64, 1_u64);
        e.properties.insert("n".into(), PropertyValue::Null);
        e.properties
            .insert("b".into(), PropertyValue::Boolean(true));
        e.properties
            .insert("i".into(), PropertyValue::Integer(-9001));
        e.properties
            .insert("f".into(), PropertyValue::Float(3.5_f64));
        e.properties
            .insert("s".into(), PropertyValue::String("héllo→".into()));
        e.properties.insert(
            "l".into(),
            PropertyValue::List(vec![
                PropertyValue::Integer(1),
                PropertyValue::String("x".into()),
            ]),
        );
        let mut inner = BTreeMap::new();
        inner.insert("k".to_string(), PropertyValue::Boolean(false));
        e.properties.insert("m".into(), PropertyValue::Map(inner));

        let want = e.properties.clone();
        let reader = round_trip(&[e], Direction::Out, 0, 0);
        let got = &reader.neighbors(0).unwrap()[0].properties;
        assert_eq!(got, &want);
    }

    #[test]
    fn source_out_of_band_is_rejected() {
        let reader = round_trip(&[], Direction::Out, 10, 20);
        let err = reader.degree(9).unwrap_err();
        assert!(matches!(err, StorageFormatError::SourceOutOfBand { .. }));
        let err = reader.expand(21, ExpandCap::unbounded()).unwrap_err();
        assert!(matches!(err, StorageFormatError::SourceOutOfBand { .. }));
    }

    // ---- Bounded-GET / r<=1 access pattern ----

    #[test]
    fn expansion_issues_at_most_two_range_gets() {
        let edges: Vec<Edge> = (0..50)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (100 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        let exp = reader.expand(1, ExpandCap::unbounded()).unwrap();
        // One GET for the directory entry + one for the neighbour block = 2.
        assert_eq!(
            exp.gets, 2,
            "a hop must be a bounded batch, not per-edge GETs"
        );
        assert_eq!(exp.neighbors.len(), 50);
        assert!(!exp.truncated);
    }

    #[test]
    fn empty_source_costs_one_get_no_block_fetch() {
        let reader = round_trip(&[], Direction::Out, 0, 0);
        let exp = reader.expand(0, ExpandCap::unbounded()).unwrap();
        assert_eq!(exp.gets, 1, "no edges => only the directory probe");
        assert!(exp.neighbors.is_empty());
    }

    // ---- Early-abort hard byte/degree cap (ADR 0008 §3.4, C2) ----

    #[test]
    fn neighbor_count_cap_truncates_early() {
        let edges: Vec<Edge> = (0..1000)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (10_000 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        let cap = ExpandCap {
            max_bytes: usize::MAX,
            max_neighbors: 10,
        };
        let exp = reader.expand(1, cap).unwrap();
        assert_eq!(exp.neighbors.len(), 10, "LIMIT-10 must stop at 10");
        assert!(exp.truncated);
    }

    #[test]
    fn byte_cap_never_fetches_beyond_cap_for_a_super_hub() {
        // A high-degree "super hub" source: many neighbours => a big block.
        let edges: Vec<Edge> = (0..5000)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (1_000_000 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);

        let full_degree = reader.degree(1).unwrap();
        assert_eq!(full_degree, 5000);

        // Cap the read at a small budget; the directory `block_len` is seen
        // before the bytes, so we must never fetch more than the cap.
        let cap = ExpandCap::bytes(256);
        let exp = reader.expand(1, cap).unwrap();
        assert!(
            exp.bytes_read <= 256 + DIR_ENTRY_LEN,
            "realized bytes {} exceeded cap (256 + dir entry {})",
            exp.bytes_read,
            DIR_ENTRY_LEN
        );
        assert!(
            exp.truncated,
            "a capped super-hub read must report truncation"
        );
        // It still decoded *some* valid prefix of neighbours.
        assert!(!exp.neighbors.is_empty());
        assert!(exp.neighbors.len() < 5000);
        // Every decoded neighbour is a real, in-order target.
        for w in exp.neighbors.windows(2) {
            assert!(w[0].neighbor.get() <= w[1].neighbor.get());
        }
    }

    #[test]
    fn byte_cap_equal_to_block_reads_everything() {
        let edges: Vec<Edge> = (0..20)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (100 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        let (_, block_len, _, _) = reader.read_dir_entry(1).unwrap();
        let exp = reader
            .expand(1, ExpandCap::bytes(block_len as usize))
            .unwrap();
        assert_eq!(exp.neighbors.len(), 20);
        assert!(!exp.truncated);
    }

    // ---- BUG-0028: byte cap below the block's leading degree varint ----
    //
    // The remaining byte budget handed to the last source(s) of a frontier can
    // legitimately be 0 or a few bytes (§3.4 / C2 budget-driven early-abort).
    // A cap that lands *inside* (or before) the leading `degree` varint must
    // still early-abort cleanly — `Ok(truncated, neighbors: [])` — never `Err`.

    #[test]
    fn byte_cap_zero_early_aborts_cleanly() {
        let edges: Vec<Edge> = (0..40)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (100 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        // n = 0: we ask for zero bytes of the block. This is a clean early-abort,
        // NOT a BadVarint error.
        let exp = reader
            .expand(1, ExpandCap::bytes(0))
            .expect("max_bytes=0 must early-abort, not error");
        assert!(exp.neighbors.is_empty(), "no bytes => no neighbours");
        assert!(exp.truncated, "a zero-byte cap is a truncation");
    }

    #[test]
    fn byte_cap_one_on_multibyte_degree_prefix_early_aborts() {
        // 200 neighbours => leading degree varint is 2 bytes (200 >= 128).
        let edges: Vec<Edge> = (0..200)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (100 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        assert_eq!(reader.degree(1).unwrap(), 200);
        // n = 1: the 2-byte degree varint cannot be read from a 1-byte buffer.
        // That is the early abort, not corruption.
        let exp = reader
            .expand(1, ExpandCap::bytes(1))
            .expect("max_bytes=1 below a 2-byte degree varint must early-abort");
        assert!(exp.neighbors.is_empty());
        assert!(exp.truncated);
    }

    #[test]
    fn byte_cap_sweep_is_monotone_and_error_free() {
        // A real block; sweep the cap from 0 up to and past the full block.
        let edges: Vec<Edge> = (0..200)
            .map(|i| Edge::new(i as u64, "FOLLOWS", 1_u64, (100 + i) as u64))
            .collect();
        let reader = round_trip(&edges, Direction::Out, 1, 1);
        let (_, block_len, degree, _) = reader.read_dir_entry(1).unwrap();
        let block_len = block_len as usize;
        let degree = degree as usize;

        let mut prev = 0usize;
        for n in 0..=(block_len + 4) {
            let exp = reader
                .expand(1, ExpandCap::bytes(n))
                .unwrap_or_else(|e| panic!("max_bytes={n} must early-abort, got Err({e:?})"));
            // Monotone: a larger byte cap never yields fewer neighbours.
            assert!(
                exp.neighbors.len() >= prev,
                "neighbour count regressed: max_bytes={n} gave {} after {prev}",
                exp.neighbors.len()
            );
            prev = exp.neighbors.len();
            // Decoded prefix is always valid + in target order.
            for w in exp.neighbors.windows(2) {
                assert!(w[0].neighbor.get() <= w[1].neighbor.get());
            }
            // Truncation is reported iff we did not (provably) read the whole block.
            if n >= block_len {
                assert!(!exp.truncated, "full cap should not report truncation");
                assert_eq!(exp.neighbors.len(), degree);
            } else {
                assert!(exp.truncated, "cap below block_len must report truncation");
            }
        }
    }

    #[test]
    fn full_buffer_corrupt_degree_varint_fails_closed() {
        // BUG-0028 AC #2: the early-abort relaxation applies only to a *capped*
        // buffer. A genuinely corrupt FULL buffer (e.g. an unterminated degree
        // varint) must still fail closed — corruption is not silently truncated.
        // (In production `open()`'s checksum catches this; here we exercise the
        // decoder directly to pin the `buffer_capped == false` branch.)
        let corrupt = vec![0x80u8; 4]; // all continuation bits set, never ends
        let err = decode_block_prefix(&corrupt, 3, usize::MAX, false).unwrap_err();
        assert!(
            matches!(err, StorageFormatError::BadVarint),
            "full-buffer corrupt degree varint must fail closed, got {err:?}"
        );
        // Same bytes, but flagged as a capped buffer => clean early-abort.
        let (neighbors, truncated) =
            decode_block_prefix(&corrupt, 3, usize::MAX, true).expect("capped => early-abort");
        assert!(neighbors.is_empty());
        assert!(truncated);
    }

    // ---- Fail-closed framing ----

    #[test]
    fn bad_magic_fails_closed() {
        let mut store = MemoryStore::new();
        let mut bytes = AdjacencyShardWriter::new(0, Direction::Out, 0, 0).finish();
        bytes[0] ^= 0xff; // corrupt magic
        store.put(KEY, bytes).unwrap();
        let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
        assert!(matches!(err, StorageFormatError::BadMagic { .. }));
    }

    #[test]
    fn unsupported_version_fails_closed() {
        let mut store = MemoryStore::new();
        let mut bytes = AdjacencyShardWriter::new(0, Direction::Out, 0, 0).finish();
        // bump format_version (bytes 4..6) to an unknown value
        bytes[4] = 0xfe;
        bytes[5] = 0xff;
        store.put(KEY, bytes).unwrap();
        let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
        assert!(matches!(
            err,
            StorageFormatError::UnsupportedVersion { .. } | StorageFormatError::ChecksumMismatch
        ));
    }

    #[test]
    fn wrong_object_kind_fails_closed() {
        let mut store = MemoryStore::new();
        let mut bytes = AdjacencyShardWriter::new(0, Direction::Out, 0, 0).finish();
        bytes[6] = 1; // NCOL kind, not ADJ
        store.put(KEY, bytes).unwrap();
        let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
        assert!(matches!(
            err,
            StorageFormatError::WrongObjectKind { .. } | StorageFormatError::ChecksumMismatch
        ));
    }

    #[test]
    fn checksum_mismatch_fails_closed() {
        let edges = vec![Edge::new(1_u64, "FOLLOWS", 0_u64, 5_u64)];
        let mut w = AdjacencyShardWriter::new(0, Direction::Out, 0, 0);
        for e in &edges {
            w.push(e);
        }
        let mut bytes = w.finish();
        // Flip a byte in a neighbour block (after the header, before trailer).
        let mid = HEADER_LEN + 1;
        bytes[mid] ^= 0x01;
        let mut store = MemoryStore::new();
        store.put(KEY, bytes).unwrap();
        let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
        assert!(matches!(err, StorageFormatError::ChecksumMismatch));
    }

    #[test]
    fn truncated_object_fails_closed() {
        let mut store = MemoryStore::new();
        let bytes = AdjacencyShardWriter::new(0, Direction::Out, 0, 0).finish();
        store.put(KEY, bytes[..HEADER_LEN / 2].to_vec()).unwrap();
        let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
        assert!(matches!(err, StorageFormatError::Truncated { .. }));
    }

    #[test]
    fn missing_object_is_store_error() {
        let store = MemoryStore::new();
        let err = AdjacencyShardReader::open(store, "nope").unwrap_err();
        assert!(matches!(
            err,
            StorageFormatError::Store(StoreError::NotFound(_))
        ));
    }

    // ---- Byte primitives ----

    #[test]
    fn varint_round_trips() {
        for v in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut c = Cursor::new(&buf);
            assert_eq!(c.read_varint().unwrap(), v);
        }
    }

    #[test]
    fn zigzag_round_trips() {
        for v in [0i64, -1, 1, -2, 2, i64::MIN, i64::MAX, -123456789] {
            assert_eq!(zigzag_decode(zigzag_encode(v)), v);
        }
    }

    #[test]
    fn varint_overflow_fails_closed() {
        // 10 continuation bytes => shift past 64 bits.
        let buf = vec![0xff; 10];
        let mut c = Cursor::new(&buf);
        assert!(matches!(
            c.read_varint(),
            Err(StorageFormatError::BadVarint)
        ));
    }

    #[test]
    fn distinct_sources_counts_blocks() {
        let mut w = AdjacencyShardWriter::new(0, Direction::Out, 0, 9);
        w.push(&Edge::new(1_u64, "T", 0_u64, 1_u64));
        w.push(&Edge::new(2_u64, "T", 0_u64, 2_u64));
        w.push(&Edge::new(3_u64, "T", 4_u64, 5_u64));
        assert_eq!(w.distinct_sources(), 2);
    }

    #[test]
    #[should_panic(expected = "outside shard band")]
    fn push_out_of_band_panics() {
        let mut w = AdjacencyShardWriter::new(0, Direction::Out, 0, 5);
        w.push(&Edge::new(1_u64, "T", 99_u64, 1_u64));
    }
}

// ===========================================================================
// Property test (AC #3): arbitrary directed typed edge sets round-trip exactly.
//
// Uses the in-repo deterministic SplitMix64 generator rather than the `proptest`
// crate to avoid a large new license surface mid-cascade (decision 0034 #2).
// Seeded => reproducible failures.
// ===========================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::dataset::SplitMix64;
    use crate::model::Edge;
    use crate::storage::MemoryStore;

    /// One expected neighbour in the reference model: `(neighbor_id, edge_id,
    /// properties)`. The reader must reproduce this exactly.
    type RefNeighbor = (u64, u64, BTreeMap<String, PropertyValue>);

    /// A random property value up to `depth` of nesting.
    fn random_value(rng: &mut SplitMix64, depth: u8) -> PropertyValue {
        let max_tag = if depth == 0 { 6 } else { 8 };
        match rng.below(max_tag) {
            0 => PropertyValue::Null,
            1 => PropertyValue::Boolean(rng.below(2) == 1),
            2 => {
                // full i64 range including extremes
                #[allow(clippy::cast_possible_wrap)]
                let i = rng.next_u64() as i64;
                PropertyValue::Integer(i)
            }
            3 => {
                // include NaN / inf occasionally
                let f = match rng.below(5) {
                    0 => f64::NAN,
                    1 => f64::INFINITY,
                    2 => f64::NEG_INFINITY,
                    _ => rng.unit_f64() * 1e9 - 5e8,
                };
                PropertyValue::Float(f)
            }
            4 => {
                let len = rng.below(8) as usize;
                let alphabet = ['a', 'z', 'é', '→', '0', ' ', '"'];
                let s: String = (0..len)
                    .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
                    .collect();
                PropertyValue::String(s)
            }
            5 => PropertyValue::String(String::new()), // empty string edge case
            6 => {
                let n = rng.below(4) as usize;
                PropertyValue::List((0..n).map(|_| random_value(rng, depth - 1)).collect())
            }
            _ => {
                let n = rng.below(4) as usize;
                let mut m = BTreeMap::new();
                for i in 0..n {
                    m.insert(format!("k{i}"), random_value(rng, depth - 1));
                }
                PropertyValue::Map(m)
            }
        }
    }

    fn random_properties(rng: &mut SplitMix64) -> BTreeMap<String, PropertyValue> {
        let n = rng.below(4) as usize;
        let mut m = BTreeMap::new();
        for i in 0..n {
            m.insert(format!("p{i}"), random_value(rng, 2));
        }
        m
    }

    /// Compare two property values structurally, treating NaN as equal to NaN
    /// (the codec preserves the exact bits, so NaN round-trips identically; the
    /// derived `PartialEq` on `PropertyValue` already treats NaN as identical,
    /// per `value.rs`, but we assert it explicitly for clarity).
    fn assert_props_eq(a: &BTreeMap<String, PropertyValue>, b: &BTreeMap<String, PropertyValue>) {
        assert_eq!(a, b, "property map must round-trip identically");
    }

    #[test]
    fn arbitrary_edge_sets_round_trip_identically() {
        for seed in 0..200u64 {
            let mut rng = SplitMix64::new(seed.wrapping_mul(0x9E37_79B9));
            // A band [0, hi] and a random set of edges within it.
            let hi = rng.below(40) + 1; // band width 2..=41
            let n_edges = rng.below(120) as usize;

            let dir = if rng.below(2) == 0 {
                Direction::Out
            } else {
                Direction::In
            };

            let mut edges: Vec<Edge> = Vec::with_capacity(n_edges);
            for eid in 0..n_edges {
                // `block_key` is whichever endpoint indexes this shard's band
                // (source for Out, target for In); it must lie in [0, hi]. The
                // other endpoint is an arbitrary node id.
                let block_key = rng.below(hi + 1);
                let other = rng.next_u64() % 1_000_000;
                let (src, tgt) = match dir {
                    Direction::Out => (block_key, other),
                    Direction::In => (other, block_key),
                };
                let rel = ["FOLLOWS", "KNOWS", "RATED"][rng.below(3) as usize];
                let mut e = Edge::new(eid as u64, rel, src, tgt);
                e.properties = random_properties(&mut rng);
                edges.push(e);
            }

            // Build a reference model: source -> sorted Vec of (neighbor,
            // edge_id, props) — see `RefNeighbor` for the tuple's meaning.
            let mut expected: BTreeMap<u64, Vec<RefNeighbor>> = BTreeMap::new();
            for e in &edges {
                let (s, nbr) = match dir {
                    Direction::Out => (e.source.get(), e.target.get()),
                    Direction::In => (e.target.get(), e.source.get()),
                };
                expected
                    .entry(s)
                    .or_default()
                    .push((nbr, e.id.get(), e.properties.clone()));
            }
            for v in expected.values_mut() {
                v.sort_by_key(|a| (a.0, a.1));
            }

            // Write + read back.
            let mut w = AdjacencyShardWriter::new(11, dir, 0, hi);
            for e in &edges {
                w.push(e);
            }
            let bytes = w.finish();
            let key = format!("db/data/h/seed-{seed}.adj");
            let mut store = MemoryStore::new();
            store.put(&key, bytes).unwrap();
            let reader = AdjacencyShardReader::open(store, &key).unwrap();

            // Every source in the band reads back exactly the expected list.
            for s in 0..=hi {
                let got = reader.neighbors(s).unwrap();
                let want = expected.get(&s).cloned().unwrap_or_default();
                assert_eq!(
                    got.len(),
                    want.len(),
                    "seed {seed}: degree mismatch at source {s}"
                );
                assert_eq!(reader.degree(s).unwrap() as usize, want.len());
                for (g, (wn, we, wp)) in got.iter().zip(want.iter()) {
                    assert_eq!(g.neighbor.get(), *wn, "seed {seed} src {s}: neighbour id");
                    assert_eq!(g.edge_id.get(), *we, "seed {seed} src {s}: edge id");
                    assert_props_eq(&g.properties, wp);
                }
            }
        }
    }
}
