---
id: T-0007
title: Implement columnar node-property object writer + range-read reader
type: task
status: in_review
priority: P1
assignee: implementer-wf_fe688db0-093-32
epic: EPIC-001
deps: [SPIKE-0003, T-0006]
rubric_refs: [2, 3]
estimate: M
created: T0+0:20
updated: T0+3:55
---

## Context

The node side of the on-object layout: nodes stored columnar (one column per
property, sorted by node id for range access) so the planner can issue a single
large range GET over a contiguous node-id span. This is the storage-format
implementation and is **design-gated on SPIKE-0003** (the format spec), which in
turn depends on SPIKE-0001's byte budget. Stays `backlog` until SPIKE-0003 is
steering-ratified (`steering-storage`). See `EPIC-001` and the storage-format spec
owned by `SPIKE-0003` (lands under `docs/adr/`).

## Acceptance criteria
- [ ] Writer serialises a batch of nodes into columnar node-property objects per the SPIKE-0003 spec (field order, encoding, alignment, object naming).
- [ ] Reader reconstructs nodes from those objects; a `get_range` over an id span fetches only the relevant partition (verified: bytes fetched ≤ partition size, not whole-object).
- [ ] Round-trip fidelity: arbitrary node sets (proptest) serialise and read back identical (labels + all property types).
- [ ] A representative range-GET for an in-envelope node span reads ≤ B_max share allotted to the node side (per SPIKE-0001 budget) — asserted in an integration test on the mock.
- [ ] tests added (unit + integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if the format detail deviates from SPIKE-0003
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0003 (storage format spec) ratification.
Flip to `ready` only when SPIKE-0003 is `done`.
- T0+3:45 (implementer-wf_fe688db0-093-32): claimed; deps satisfied (SPIKE-0003
  done → ADR 0008 `docs/adr/0008-storage-format.md`; T-0006 done; SPIKE-0001 done).
  Branch `work/T-0007-columnar-node-property-writer-reader` off latest main
  (`d4a9c70`). Also fixed a pre-existing frontmatter typo (`status: readypriority`
  → split into `status` + `priority` lines). Implements ADR 0008 §2 (`.ncol`
  columnar node-property objects) + §2.4 columnar range-read reader over the
  `storage::ObjectStore` trait, honouring land-gate condition **C3** (a
  single-property filter read fetches ≤ that column's chunk bytes, not whole node
  records). TDD-first.
- T0+3:55 (implementer-wf_fe688db0-093-32): PR opened → `in_review`. Implemented
  `src/storage/ncol.rs` (writer + range-read reader, self-describing framing per
  ADR 0008 §2.2, fail-closed parsing), `src/storage/counting.rs`
  (`CountingStore` byte-budget instrument), `tests/ncol_columnar_read.rs`
  (C3/AC4 byte-exact integration test on the in-process backend). 259 tests
  green (28 new), doctests green, `./format_code.sh` exit 0. Env: `up.sh`
  (MinIO already up) + `bucket.sh T-0007` (`caerostris-it-t-0007`) exercised.
  All ADR 0008 §2 obligations met; C3 discharged. Branch
  `work/T-0007-columnar-node-property-writer-reader`.
- **AC3 deviation (recorded):** used a deterministic 400-seed generative
  round-trip test (repo `SplitMix64`) instead of `proptest`, which would add ~19
  transitive crates to the lean lockfile + license manifest (risking the Cat-12
  license GATE) — same call `src/dataset/rng.rs` made. Intent of AC3 (arbitrary
  node sets round-trip identically) is met. **Follow-up:** a `test-author` may
  add a proptest harness if the workspace-level dep tree is later justified.
