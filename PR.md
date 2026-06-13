# PR: BUG-0007 — 100% TCK target is ill-defined without pinned tag and pending-in-denominator rule

## Board item

[.project/board/tasks/BUG-0007-100-percent-tck-target-is-ill-defined-without-pinned-tag-and-pending-in-denominator-rule.md](.project/board/tasks/BUG-0007-100-percent-tck-target-is-ill-defined-without-pinned-tag-and-pending-in-denominator-rule.md)

## Rubric refs

Cat 4 (openCypher completeness / TCK — GATE), Cat 10 (tests/coverage).

## Acceptance criteria (from board item)

- [x] Rubric Cat. 4 and T-0002 amended to state explicitly:
      `pass_rate = pass / total`, `total = pass + pending + fail`, no scenario
      excluded from `total`; reaching 100 requires `pending == 0 && fail == 0`.
- [x] A specific openCypher TCK release tag is pinned and recorded (in T-0002 and a
      decision doc); the expected `total` scenario count for that tag is recorded.
- [x] Harness emits the pinned tag and `total` in its machine-readable output so
      the rubric grader can assert the suite was not shrunk.
- [x] A guard test fails if the loaded scenario count differs from the recorded
      pinned `total` (catches accidental or deliberate suite shrinkage).
- [x] `./format_code.sh` green.

## Summary of change

Resolves BUG-0007, filed by `steering-query-cypher`: the Cat. 4 GATE metric "100% of
the TCK" was ambiguous about its denominator and lacked a pinned suite version, making
it gameable and not a credible gate.

This PR encodes the non-gameable pass-rate contract in code (`src/tck.rs`) and updates
all specification documents to be consistent:

- `TckSummary` carries `pass`, `pending`, `fail`, and `total = pass + pending + fail`.
  `pass_rate = pass / total`; both `pending` and `fail` are in the denominator.
  `is_complete()` returns true only when `pending == 0 && fail == 0`.
- openCypher TCK release `1.0.0-M23` (commit `007895a`) is pinned with its measured
  scenario count (1615) and feature-file count (220) as named constants.
- `verify_suite_size()` guards against silent suite shrinkage or growth — the harness
  must load exactly `PINNED_TCK_SCENARIOS`.
- `TckSummary::to_json()` emits `tck_tag`, `tck_commit`, `total`, and the buckets in a
  stable shape for the rubric-grader.
- `master-rubric.md` Cat. 4, `T-0002` acceptance criteria, `testing-and-benchmarks.md`,
  `decision 0008`, and the `rubric-grader` agent are all updated consistently.

The change is additive: new `src/tck.rs` module and `tests/tck_passrate_contract.rs`
integration tests, plus documentation amendments. No existing behaviour is modified.

## Test evidence

`cargo nextest run` — all tests pass (including new `tck_passrate_contract` suite).

`./format_code.sh`: green (cargo fmt clean, `cargo clippy --all-targets -- -D warnings`
zero warnings, taplo clean).

New tests in `tests/tck_passrate_contract.rs` exercise:
- pass_rate denominator includes pending and fail
- empty suite returns 0.0 not NaN
- is_complete() only true when pending == 0 && fail == 0
- verify_suite_size() fails on count mismatch
- to_json() emits required fields (tck_tag, tck_commit, total)

## Review gate

- [x] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [x] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green
- [x] coverage not regressed
- [x] board item updated to `in_review`

<!-- Reviewers appended their verdicts below — both approved at base 3a9d645.
     Landing was previously blocked by a rebase conflict on src/lib.rs (BUG-0006
     landed concurrently and added pub mod query; at the same anchor line).
     This reland resolves the conflict (keep BOTH pub mod query; AND pub mod tck;,
     sorted) and rebases cleanly onto current main. Review sign-offs are preserved
     as confirmed by the integrator board note at T+~1:45. -->

---

### adversarial-reviewer verdict (at base 3a9d645)

verdict: approve

The tck module correctly encodes the pass-rate denominator contract. The suite-size
guard prevents gaming via suite shrinkage. Constants for pinned tag and scenario count
are the right level of rigidity. The implementation is additive and has no risk of
regressing existing behaviour. No findings.

---

### premortem-analyst verdict (at base 3a9d645)

verdict: approve

Failure modes considered: (1) future TCK version bump causing verify_suite_size() to
always fail — mitigated by the constant being named and easy to update with a deliberate
decision; (2) pending-stuffing to avoid is_complete() — mitigated by pending being in
the denominator; (3) rubric-grader reading wrong field — mitigated by to_json() emitting
a stable, documented shape. No unmitigated blockers.
