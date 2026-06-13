---
id: BUG-0021
title: TCK outline substitution chains instead of single-pass — a value containing a sibling column's <token> is re-substituted (latent; 0 hits in 2024.3 corpus)
type: bug
status: in_review
priority: P3
assignee: test-author (wf_156e2b80-bb6-51)
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
created: T0+3:29
updated: T0+4:12
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
- T0+3:50 (adversarial-reviewer, reviewing PR on
  `work/BUG-0021-tck-outline-single-pass-substitution`):
  **changes_requested.** The single-pass scanner fix anchors on any `<`,
  including Cypher comparison operators (`<`, `<=`), so a comparison `<` that
  precedes a real `<placeholder>` swallows or drops the placeholder. Verified
  against the real `expand_scenario` over the pinned corpus: **68 expanded
  scenarios across 6 feature files (Comparison2, Quantifier7/9/10/11/12) now
  leave a raw `<lhs>`/`<rhs>`/`<predicate>`/`<operands>` token in the query** —
  e.g. Comparison2 `[5] Comparing NaN` yields `... 0.0 / 0.0 < <rhs> AS lt`
  where the old code produced `... < 1 ...`. This is an *active* Cat. 4 GATE
  regression, strictly worse than the original (0-impact) defect. The
  reconciliation guard `scenario_has_placeholder` shares the same `<`-anchoring
  blind spot and does NOT catch it (still green). Fix: substitute only **bound**
  `<header>` tokens (longest-match against the binding keys), not "the next
  `<...>` span"; and harden the guard to scan from every `<`. Add a
  comparison-operator-adjacent regression test. Verdict + evidence in the
  worktree's PR.md.
- T0+4:12 (integrator): RELAND BLOCKED — adversarial-reviewer sign-off is
  `changes_requested` (not `approve`). The current implementation in
  `work/BUG-0021-tck-outline-single-pass-substitution` anchors on raw `<` which
  collides with Cypher comparison operators; 68 expanded scenarios are corrupted.
  Review-gate checkboxes in PR.md are both unchecked. Branch returned to author.
  Required fix: substitute only bound `<header>` tokens by scanning the binding
  keys directly (not arbitrary `<...>` spans); add comparison-operator regression
  test; re-run format+tests; reset review gate for a fresh review pass. Status
  set to `blocked`.
- T0+4:01 (test-author, wf_156e2b80-bb6-51): claimed; fixed TDD-first on
  `work/BUG-0021-tck-outline-substitution-chains-instead-of-single-` (based on
  latest main `ca73710`). **Path note:** BUG-0009 landed into main as
  `tck-runner/src/outline.rs` (not `expand.rs`), so the defective `substitute`
  lives in `outline.rs`; the reconciliation guard the AC names
  (`expanded_denominator_is_pinned_and_reconciled`) is `outline_expansion_total_is_reconciled`
  in `tck-runner/tests/vendored_corpus.rs`. Same defect, same contract.
  Replaced the chained `String::replace` loop with a single-pass left-to-right
  scanner (longest-token match at each `<`, verbatim copy otherwise). RED test
  `substitution_is_single_pass_not_chained` reproduced `RETURN x AND x`; after
  the fix it expands to `RETURN <b> AND x`. Added substring-collision (`<n>` vs
  `<name>`) and unbound-placeholder guards. Full tck-runner suite green
  (incl. `outline_expansion_total_is_reconciled`, `corpus_expands_to_expected_total`
  — 3884 denominator unchanged); workspace `cargo nextest run` = 278/278 pass;
  `./format_code.sh` green (clippy clean). PR worktree commit 29fa955. → in_review.
- T0+4:0X (adversarial-reviewer, reviewing this PR): **APPROVE.** Verified the
  single-pass scanner against 12 hand-built adversarial probes beyond the 3
  shipped tests — intra-text injection collision (`<a>`->`<b`,`<b>`->`Z` => `<bZ`,
  the strongest single-pass proof), multibyte UTF-8 after a bare `<`, trailing `<`,
  empty `<>` token, longest-vs-shorter fallback, empty value, no bindings, and
  Cypher `<=`/`<>` operators in query text — all correct, no panic, no char-boundary
  break. Confirmed the regression test is RED on the old chained loop (`RETURN x AND x`)
  and GREEN on the fix. tck-runner 43 lib + 6 corpus tests green incl. the unchanged
  3884 denominator; `./format_code.sh` exit 0. Branch based on `ca73710`; main is now
  `ec47614` but touched nothing in `tck-runner/` since merge-base, so the rebase is
  clean. No blocking findings. Adversarial-reviewer box ticked in PR.md; awaits
  premortem-analyst sign-off before the integrator lands.
- T0+3:5X (premortem-analyst, reviewing this PR): **APPROVE — no blocking failure
  modes.** Independently reproduced the bug on the OLD chained `substitute`
  (`RETURN x AND x`) vs the NEW scanner (`RETURN <b> AND x`) — regression test is a
  genuine RED→GREEN guard, and the new scanner is order-independent (strict
  robustness gain). Proved the hand-rolled scanner always advances `i` (token len
  ≥2 or char width ≥1) so no infinite loop — probed the empty-`<>`-self-inject worst
  case, terminates. No UTF-8 char-boundary panic (`<` is ASCII; verbatim branch
  advances by full char width). 3884 denominator unmoved
  (`outline_expansion_total_is_reconciled` + `corpus_expands_to_expected_total`
  green) so the Cat. 4 GATE pass-rate cannot be inflated/shrunk. No new deps
  (`Cargo.toml`/`Cargo.lock` unchanged), no `unsafe`, no ACID/S3/latency/concurrency
  surface — test-only parse-time helper. `./format_code.sh` exit 0;
  `cargo test -p tck-runner` lib 43/43 + corpus 6/6 green. **Non-blocking
  operational note for the integrator:** a second, *unreviewed* BUG-0021 branch
  exists (`work/BUG-0021-tck-outline-single-pass-substitution`, wf_e9fceb87-27c-43,
  no sign-offs) — land THIS canonical branch and drop the duplicate. Premortem box
  ticked in PR.md. Both gates now green; ready for the integrator.
