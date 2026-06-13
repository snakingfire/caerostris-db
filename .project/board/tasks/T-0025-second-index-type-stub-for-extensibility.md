---
id: T-0025
title: Stub a second index type against the trait to prove extensibility
type: task
status: in_review
priority: P3
assignee:
epic: EPIC-005
deps: [T-0022]
rubric_refs: [5]
estimate: S
created: T0+0:20
updated: T+4:10
---

## Context

Cat. 5 = 100 requires a second index type stubbed against the same trait to prove
the interface is not leaking B-tree specifics. A range index (or a stub full-text
index) implemented against T-0022's trait demonstrates extensibility without a core
rewrite. Can be done as soon as the trait (T-0022) exists. See `EPIC-005`.

## Acceptance criteria
- [x] A second index type (range index or stub full-text) implements the T-0022 trait. — `HashIndex<K,V>` (equality-only hash multimap) in `src/index/hash.rs`.
- [x] The implementation required no change to the trait signature (proves the interface generalises) — documented. — trait signatures untouched; only additive `Hash` on `OrderedKey`. ADR 0005 § Extensibility demonstration.
- [x] The planner can consult it through the same trait-based API as the B-tree. — gains `PropertyIndex` facade via the existing blanket impl; tested behind `&dyn`/`Box<dyn>` and in a mixed B-tree+hash catalog.
- [x] tests added (unit covering the second index's trait conformance); coverage not regressed — 25 new tests in `src/index/hash_tests.rs` (231 → 256).
- [x] docs / ADR updated noting the extensibility demonstration — `docs/adr/0005-pluggable-index-interface.md`.
- [x] `./format_code.sh` green

## Notes / log
Ready once T-0022 lands. P3 (extensibility proof) — pull after the B-tree path if
agents are free; cheap and de-risks the Cat. 5 = 100 anchor.

T+3:42 — implementer-wf_e9fceb87-27c-44 claimed. T-0022 is `done` (commit ab5fc7a):
the `SecondaryIndex` trait + `PropertyIndex` facade + blanket impl exist in
`src/index/mod.rs`. T-0025 promotes it to a first-class library index type
(`HashIndex`) against the same trait — no signature change.

T+3:44 — implemented TDD-first on `work/T-0025-second-index-type-stub`:
- `src/index/hash.rs`: `HashIndex<K, V>` — equality-only hash multimap.
- `src/index/hash_tests.rs`: 25 unit tests.
- ADR 0005 § Extensibility demonstration.

T+4:10 — Two T-0025 implementations exist in flight:
  1. `work/T-0025-second-index-type-stub-for-extensibility` (FullTextIndex) — adversarial-reviewer returned `changes_requested` (T+3:55). Still blocked.
  2. `work/T-0025-second-index-type-stub` (HashIndex) in `.claude/worktrees/wf_e9fceb87-27c-44` — adversarial reviewer found blockers (BUG-0020 conflict, stale rebase). Reland dispatched.
