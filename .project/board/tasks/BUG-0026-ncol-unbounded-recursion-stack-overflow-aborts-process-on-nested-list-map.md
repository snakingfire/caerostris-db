---
id: BUG-0026
title: .ncol encode/decode of nested List/Map recurses unbounded — deep nesting aborts the process (SIGABRT), not fail-closed
type: bug
status: in_progress
priority: P1
assignee: implementer-wf_3215ee4a-fcf-27
epic: EPIC-001
deps: [T-0007]
rubric_refs: [2, 1, 10]
estimate: S
created: T0+~3:58
updated: T0+~4:27
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
