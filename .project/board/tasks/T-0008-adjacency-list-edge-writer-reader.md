---
id: T-0008
title: Implement compressed adjacency-list edge writer + chunked range reader
type: task
status: in_review
priority: P1
assignee: implementer-wf_fe688db0-093-33
epic: EPIC-001
deps: [SPIKE-0003, T-0006]
rubric_refs: [2, 3]
estimate: M
created: T0+0:20
updated: T0+3:55
---

## Context

The edge side of the layout: edges stored as compressed adjacency lists sorted by
source node id, chunked so a hop expansion fetches only the frontier's adjacency
without reading the whole edge set. Per `SPIKE-0008` falsification F1, adjacency
chunking must allow **early-abort partial reads** so the binding 50 Mbps / 4 MB
case is feasible. Design-gated on SPIKE-0003. See `EPIC-001` and the storage-format
spec owned by `SPIKE-0003` (ADR 0008, `docs/adr/0008-storage-format.md`).

## Acceptance criteria
- [ ] Writer serialises edges into source-sorted, chunked adjacency-list objects per the SPIKE-0003 spec; chunk granularity matches the spec's range-read access pattern.
- [ ] Reader expands the out-neighbours of a given node id via a bounded range GET; supports early-abort (stop fetching once the LIMIT-driven frontier is satisfied) per SPIKE-0008 F1.
- [ ] Round-trip fidelity: arbitrary directed typed edge sets (proptest) serialise and read back identical, including edge properties.
- [ ] A single-hop expansion for an in-envelope frontier reads ≤ its allotted B_max share (per SPIKE-0001) — asserted on the mock.
- [ ] tests added (unit + integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if format detail deviates from SPIKE-0003
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0003. Coordinate chunk sizing with the
cost-model bytes budget from SPIKE-0001 / BUG-0004.
- T0+3:50 (implementer-wf_fe688db0-093-33): claimed. SPIKE-0003 done (ADR 0008
  ratified, commit 3f14a1e); T-0006 model types done. Implementing ADR 0008 §3 CSR
  adjacency writer/reader against the `ObjectStore` trait: source-sorted banded
  `.adj` shards with an intra-object fixed-stride offset directory
  (block_off, block_len, degree), delta-varint neighbor lists, columnar edge
  property blocks, BLAKE3 trailer self-checksum, fail-closed framing. Reader: O(1)
  directory indexing + bounded range-GET expansion with the §3.4 hard per-GET
  byte/degree cap (early-abort, ADR 0008 C2 / SPIKE-0008 F1). Branch
  work/T-0008-adjacency-edge-writer-reader off main 61ffdac.
  Co-located dst projection (§3.3) and the cross-shard manifest partition map
  (§5.1) are owned by T-0009/planner and are out of scope here — this task owns
  the on-`.adj`-bytes format + the single-shard banded reader with early-abort.
- T0+3:55 (implementer-wf_fe688db0-093-33): PR opened; → in_review. Branch
  `work/T-0008-adjacency-edge-writer-reader`. AdjacencyShardWriter +
  AdjacencyShardReader landed in `src/storage/adjacency.rs` with 25 module tests
  (incl. a 200-seed SplitMix64 property test) + 7 integration tests in
  `tests/adjacency_storage.rs`; full suite 280 passed, 0 skipped;
  `./format_code.sh` green (fmt + clippy -D warnings + taplo). ADR 0008 updated
  with the T-0008 Implementation-notes addendum; decision 0034 records the two
  dependency-free choices (FNV-1a trailer checksum, SplitMix64 property gen) vs.
  pulling blake3/proptest mid-cascade. Awaiting adversarial-reviewer +
  premortem-analyst sign-off, then integrator.
