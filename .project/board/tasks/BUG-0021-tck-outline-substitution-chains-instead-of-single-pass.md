---
id: BUG-0021
title: TCK outline substitution chains instead of single-pass — a value containing a sibling column's <token> is re-substituted (latent; 0 hits in 2024.3 corpus)
type: bug
status: in_review
priority: P3
assignee: test-author (wf_e9fceb87)
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
created: T0+3:29
updated: T0+3:55
---

## Context

Found during adversarial review of BUG-0009
(`work/BUG-0009-expand-outlines-per-examples-row`, the Scenario-Outline
expansion fix).

`tck-runner/src/expand.rs::substitute` applies each `(<col>, value)` binding
sequentially with `String::replace` over the **accumulating** result string:

```rust
fn substitute(text: &str, bindings: &[(String, &str)]) -> String {
    let mut result = text.to_string();
    for (token, value) in bindings {
        if result.contains(token.as_str()) {
            result = result.replace(token.as_str(), value);
        }
    }
    result
}
```

Because the loop re-scans `result` after each replacement, a value injected by an
earlier binding is itself eligible for substitution by a later binding. Correct
Cucumber/Gherkin `Scenario Outline` semantics are a **single simultaneous pass**:
a column value is placed verbatim and never re-scanned for further `<token>`s.
The module docstring explicitly claims to "mirror Cucumber's outline semantics,"
so this is a documented-contract deviation, not just an undocumented edge.

## Reproduction (adversarial probe, verified)

Outline columns `a`, `b` with one data row `a = "<b>"`, `b = "x"`, query
`RETURN <a> AND <b>`:

- **Correct (Cucumber):** `RETURN <b> AND x` — `<a>` becomes the literal text
  `<b>`; `<b>` (the original token) becomes `x`; the injected `<b>` is NOT
  re-substituted.
- **Actual (this code):** `RETURN x AND x` — `<a>` becomes `<b>`, then the later
  `<b>` binding re-substitutes the just-injected `<b>` to `x`. Corrupt.

(Substring-collision between column names — e.g. `n` vs `name` — is **not**
affected, because the `<...>` delimiters make `<n>` a non-substring of `<name>`.
Verified: that case substitutes correctly.)

## Severity / why P3 (not a BUG-0009 blocker)

- **Zero impact on the pinned `2024.3` corpus.** An exhaustive scan of all 276
  outlines (using the real `gherkin` 0.16 parse) found **0** `Examples` data
  cells whose text contains a `<token>` matching a sibling column header — the
  exact precondition for the bug. So today every substituted query is correct and
  the 3884 denominator is unaffected.
- It is a **latent** robustness defect: it could activate on a future corpus bump
  (or a hand-written fixture) that places a sibling column's `<token>` inside an
  `Examples` cell value, silently producing a corrupt query → a false `fail` or
  wrong result once a real engine (EPIC-002) runs the variant. That is the exact
  failure class BUG-0009 itself was filed to prevent, so it is worth closing
  before the engine lands and the pass-rate starts to climb.

## Acceptance criteria

- [ ] `substitute` (or `instantiate`) performs a **single simultaneous pass**:
      each `<...>` span in the source text is resolved to its column value (or
      left verbatim if unbound) without re-scanning injected values. A regex
      single-replace over `<\w...>` spans, or a manual one-pass scanner, both
      work; choose the simplest.
- [ ] Regression test: outline with columns `a`, `b`, data row `a = "<b>"`,
      `b = "x"`, query `RETURN <a> AND <b>` expands to `RETURN <b> AND x`
      (not `RETURN x AND x`).
- [ ] Existing `expand.rs` tests still pass; corpus reconciliation
      (`expanded_denominator_is_pinned_and_reconciled`) still passes (count is
      unchanged — this is a substitution-fidelity fix, not a count change).
- [ ] `./format_code.sh` green.

## Notes / log

- T0+3:29 (adversarial-reviewer, reviewing BUG-0009): filed as a non-blocking
  follow-up. BUG-0009 is approved to land because this defect corrupts no
  scenario in the pinned corpus and does not affect the denominator; it is a
  latent correctness edge to close before EPIC-002's engine exercises the
  substituted variants. Relates to BUG-0009, Decision 0013.
- T0+3:55 (test-author, wf_e9fceb87): claimed; fixed TDD-first on
  `work/BUG-0021-tck-outline-single-pass-substitution` (based on latest main
  d4a9c70). `expand.rs::substitute` is now `outline.rs::substitute` after the
  BUG-0009 land; same chained-replace defect. Rewrote it as a single-pass
  scanner over `<...>` spans (verbatim copy, no re-scan). Added 4 unit tests
  (2 reproduced the bug RED, pass GREEN). Workspace: 296/296 green; corpus
  reconciliation unchanged (total=3884); outline.rs coverage 99.47%;
  format_code.sh green. PR.md filed; status -> in_review. Awaiting
  adversarial-review + pre-mortem.
