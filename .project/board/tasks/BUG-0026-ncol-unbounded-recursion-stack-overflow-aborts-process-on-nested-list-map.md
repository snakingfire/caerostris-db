---
id: BUG-0026
title: .ncol encode/decode of nested List/Map recurses unbounded — deep nesting aborts the process (SIGABRT), not fail-closed
type: bug
status: in_review
priority: P1
assignee: implementer-wf_3215ee4a-fcf-27
epic: EPIC-001
deps: [T-0007]
rubric_refs: [2, 1, 10]
estimate: S
created: T0+~3:58
updated: T0+~4:45
---

## Context

Found during the adversarial review of T-0007 (`work/T-0007-columnar-node-property-writer-reader-wf156`).

`src/storage/ncol.rs` `encode_value` (writer) and `decode_value` (reader) recurse
once per nesting level for `PropertyValue::List` / `PropertyValue::Map` with **no
depth bound**. A property value nested deep enough overflows the stack and the
process aborts with `fatal runtime error: stack overflow` (SIGABRT) — it does not
return `NcolError::Corrupt`.

Reproduced on the branch (2 MB test-thread stack):
- Writer: survives depth 1000 (9128-byte object), aborts somewhere in 1000..5000.
- Reader: aborts decoding an object whose `k` column holds a depth-~8000 nested list.

Two distinct impacts:

1. **Reader is not fail-closed on untrusted bytes.** ADR 0008 §8.2 and the
   BUG-0014 lesson require the reader to *fail-closed* (return an error) on a
   malformed/hostile object, "never fail-open … rather than mis-reading bytes."
   A crafted/corrupt `.ncol` chunk with deep nesting makes `read_nodes` /
   `read_column` **abort the whole process** instead of returning an error. On a
   server (writer-master serving reads) that is a remote, unauthenticated DoS via a
   poisoned object; embedded, it kills the host process. The PR's own claim — "the
   reader fail-closes on … truncation" — does not cover unbounded recursion depth.
2. **Writer aborts on legitimate ingest.** A moderately nested list/map produced by
   an openCypher literal or a generated dataset (depth in the low thousands) aborts
   ingest mid-transaction. There is no recovery path; the process dies.

The generative round-trip test in `ncol.rs` caps `gen_value` nesting at `depth = 2`,
so the existing "proptest-equivalent" suite is constructed in a way that cannot
surface this failure mode.

## Acceptance criteria
- [ ] `decode_value` enforces an explicit maximum nesting depth and returns
      `NcolError::Corrupt` (fail-closed) when exceeded — never overflows the stack.
- [ ] `encode_value` either enforces the same bound (rejecting over-deep input with
      a typed error) or is made iterative; ingest of an over-deep value returns an
      error, never aborts the process.
- [ ] A test serialises/reads a value nested past the bound and asserts a typed
      error (no panic/abort). A test confirms a value at the bound round-trips.
- [ ] Depth bound documented (ADR 0008 §2.3 or the module docs) so it is a stated
      format contract, not an accident of the platform stack size.
- [ ] tests added; coverage not regressed; `./format_code.sh` green.

## Notes / log
Filed by adversarial-reviewer during T-0007 review. The columnar access pattern
(C3/AC2/AC4), round-trip fidelity for non-pathological values, and fail-closed
behaviour for bad-magic/version/truncation are all sound; this is a bounded,
self-contained robustness/DoS gap in the value codec shared by writer and reader.

- **T0+~4:38 — implementer-wf_3215ee4a-fcf-27.** Fixed on branch
  `work/BUG-0026-ncol-encode-decode-of-nested-list-map-recurses-unb` (based on
  latest main, commit 19f3edd which contains the T-0007 landing). Threaded an
  explicit nesting `depth` through `encode_value` / `decode_value` and reject
  containers past `MAX_NESTING_DEPTH = 64` with a typed
  `NcolError::NestingTooDeep`, **before** recursing (stack can never grow past
  the bound). Documented the bound as a format contract in ADR 0008 §2.3.
  Fix commit `558b251`. Tests: 6 new (TDD: RED→GREEN), `.ncol` module 25 passed,
  whole crate 464 passed, clippy clean, `./format_code.sh` exit 0.
- While here, found the **identical unbounded-recursion defect in the `.adj`
  edge-property codec** (`src/storage/adjacency.rs`, T-0008). Out of scope for
  this bug (BUG-0026 names only `ncol.rs`); filed as **BUG-0030** to keep this
  diff small. Status set to `in_review`; awaiting adversarial-reviewer +
  premortem-analyst sign-off.
- **T0+~4:40 — adversarial-reviewer: APPROVE.** Eight attacks attempted (bound
  off-by-one, 100k-deep reader stack-exhaustion, poisoned-column reader path,
  writer mid-ingest abort, wire-format drift, ACID/latency/split-brain surface,
  non-exhaustive-match break, guardrails) — all survived; verdict block + attack
  log in `PR.md`. Independently re-ran `cargo test --lib storage::ncol` (25
  passed incl. the 6 regressions + the 100k-deep stream), `clippy -D warnings`
  (clean), `cargo fmt --all --check` (exit 0). All five acceptance criteria met
  with evidence. Reviewer gate ticked. One out-of-scope, non-regression finding
  (a width/allocation DoS in the same reader, pre-existing on `main`) filed as
  **BUG-0031** — not blocking BUG-0026. Awaiting premortem-analyst sign-off
  before the integrator lands.
- **T0+~4:45 — premortem-analyst: APPROVE.** Ran the pre-mortem assuming a
  poisoned `.ncol` object took down a read-serving server in prod. Independently
  re-verified the mitigation (not on the author's word): `cargo test --lib
  storage::ncol` = 25 passed incl. the 100k-deep hostile-stream test returning a
  typed error with **no SIGABRT**; grep-confirmed `decode_column →
  decode_value(.., 0)` is the sole value-materializing reader path (no bypass);
  `cargo nextest run` = 464 passed / 0 skipped; `clippy --all-targets -D warnings`
  = exit 0; `./format_code.sh` = exit 0. No P0 failure mode left unmitigated:
  wire format byte-identical for in-bound values (no corruption/migration risk),
  zero ACID/latency/concurrency surface, writer fails closed before any write (no
  orphaned-object window). Non-blocking notes recorded in `PR.md`: the over-strict
  64-bound tradeoff (documented, strictly safer than SIGABRT), the owned
  out-of-scope siblings (BUG-0030 `.adj`, BUG-0031 width-DoS), and a flag to the
  integrator that two competing BUG-0026 branches exist — land exactly one. Both
  review-gate boxes now ticked; clear to land.
