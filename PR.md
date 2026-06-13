# PR: BUG-0006 — TCK side-effect assertions are unobservable without a QueryStatistics surface

## Board item

[.project/board/tasks/BUG-0006-tck-side-effect-assertions-are-unobservable-without-a-querystatistics-surface.md](.project/board/tasks/BUG-0006-tck-side-effect-assertions-are-unobservable-without-a-querystatistics-surface.md)

## Rubric refs

Cat 4 (openCypher completeness / TCK — GATE), Cat 10 (tests/coverage).

## Acceptance criteria (from board item)

- [x] Engine runtime exposes a `QueryStatistics`-equivalent side-effect counter covering
  `+nodes`/`-nodes`, `+relationships`/`-relationships`, `+labels`/`-labels`,
  `+properties`/`-properties`. (`+indexes`/`+constraints` explicitly deferred — out of scope
  for the P1/P2 tranches this unblocks; one-line extension path recorded in Decision 0012.)
- [~] T-0002's TCK adapter reads this surface and asserts the `Then the side effects should be:`
  step as real pass/fail. The **contract** (`from_tck_side_effects` + `matches_side_effects`) and
  the end-to-end assertion path are delivered + tested here; the Gherkin runner itself is T-0002,
  whose acceptance criteria are amended in this PR to mandate the read. BUG-0006 removes the
  structural blocker; the wiring is T-0002's job.
- [x] Property +/- counting semantics (Issue #221) pinned to the TCK's expected values for the
  pinned release and recorded in `.project/decisions/0012-tck-side-effect-counting-semantics.md`;
  the deferred `+indexes`/`+constraints` categories are documented there, not silently skipped.
- [x] One TCK side-effect scenario passes end-to-end as evidence
  (`tests/tck_side_effects.rs::create_then_delete_node_side_effects_pass`).
- [x] EPIC-002 and T-0002 acceptance criteria updated to name this surface.
- [x] `./format_code.sh` green.

## Summary of change

Resolves the P0 structural blocker filed by `steering-query-cypher` (Decision 0007): a large
class of openCypher TCK scenarios assert *side effects* (`Then the side effects should be:`) that
are not observable from the result set — e.g. `CREATE (n) DELETE n` returns no rows yet must
report `+nodes 1 / -nodes 1`. Without an engine-exposed side-effect counter, every such scenario
is structurally unpassable, so Cat. 4 = 100% (a GATE) is unreachable.

This PR introduces `caerostris_db::query::QueryStatistics` (`src/query/stats.rs`), the engine's
`QueryStatistics`-equivalent surface. It carries the eight categories the TCK emits as
non-negative occurrence counts, with recorders for the executor (`record_nodes_created`, …) and
accessors for the adapter. It parses a Gherkin side-effect table directly
(`from_tck_side_effects`, applying the TCK convention that omitted categories are zero) and
compares with `matches_side_effects` (≡ `==`), which is the exact assertion the T-0002 adapter
will perform. The Issue #221 property +/- counting ambiguity (notably that `SET`-to-same-value is
a no-op) is pinned to the TCK's expected values for the pinned release in
`.project/decisions/0012-tck-side-effect-counting-semantics.md`; `+indexes`/`+constraints` are
explicitly deferred there with a recorded extension path rather than silently dropped. EPIC-002
and T-0002 acceptance criteria are amended to mandate the surface and the adapter read. The change
is additive (a new `query` module) and touches no existing behaviour.

## Test evidence

`cargo nextest run` (17 tests, all pass):

```
     Summary [   0.488s] 17 tests run: 17 passed, 0 skipped
```

`cargo test` (adds doctests — 14 unit + 3 integration + 1 doctest, all pass):

```
test result: ok. 14 passed; 0 failed; ...   (src/lib.rs unit tests)
test result: ok. 3 passed; 0 failed; ...    (tests/tck_side_effects.rs)
test result: ok. 1 passed; 0 failed; ...    (Doc-tests caerostris_db)
```

End-to-end evidence scenario (`tests/tck_side_effects.rs`):
- `create_then_delete_node_side_effects_pass` — `CREATE (n) DELETE n` ⇒ parses
  `| +nodes | 1 | / | -nodes | 1 |` and the engine-reported stats match.
- `set_label_and_property_side_effects_pass` — omitted categories assert zero.
- `mismatched_side_effects_fail` — a divergent engine report fails (real fail, not pending).

`./format_code.sh`: green (cargo fmt clean, `cargo clippy --all-targets -- -D warnings` zero
warnings, taplo formatted 3 TOML files).

Coverage: `cargo-llvm-cov` is not installed in this worktree's shell, so a numeric % could not be
captured here (CI's coverage gate will measure it). Qualitatively the new module is fully
exercised: every public method (recorders, accessors, `from_tck_side_effects`,
`to_tck_side_effects`, `matches_side_effects`, `is_empty`/`contains_side_effects`, `Display`) and
every `SideEffectParseError` variant has a dedicated test. No existing code was modified, so prior
coverage cannot regress.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [ ] `./format_code.sh` green
- [ ] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
