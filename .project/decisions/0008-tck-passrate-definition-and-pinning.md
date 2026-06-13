# Decision 0008 — TCK pass-rate definition (pending in denominator) and release pinning

- **Date / marker:** T0 (2026-06-13T18:24:00Z)
- **Owner:** steering-query-cypher
- **Status:** recorded; tracked by BUG-0007 (P0). **Numeric pin SUPERSEDED by
  Decision 0034** (T+4:30, BUG-0018): the canonical pin is the vendored `2024.3` /
  `677cbaf` corpus with an expanded `total` of `3884`, not `1.0.0-M23` / `1615`.
  The pass-rate **definition** below (`pass / total`, nothing excluded, 100 ⇒
  `pending == 0 && fail == 0`) is unchanged and still binding.
- **Rubric:** Cat. 4 (openCypher/TCK), Cat. 10

## Context

Cat. 4 scores "TCK pass-rate %" with a 100% GATE bar. T-0002 emits
`{ total, pass, pending, fail, pass_rate: P/N }` with unimplemented features as
`pending`.

## Finding

Two ambiguities make "100%" gameable:

1. **Denominator.** "pass-rate" + a separate `pending` bucket invites
   `pass_rate = pass / (pass + fail)`, excluding `pending`. That hides
   incompleteness and is a curated subset by another name — a falsification of
   "100% means all of it, not a subset" (commanders-intent.md L31).
2. **Suite identity / drift.** "100% of the TCK" is undefined without a pinned
   release tag and a recorded `total`; otherwise the score can rise by dropping
   `.feature` files.

## Decision

- Mandate **`pass_rate = pass / total`, `total = pass + pending + fail`**, no
  scenario excluded from `total`; 100 requires `pending == 0 && fail == 0`.
  Moving a scenario to `pending` to inflate the rate is forbidden.
- Pin a specific openCypher TCK release tag; record the tag and its expected
  `total` scenario count. Harness emits both; a guard test fails if the loaded
  count differs from the recorded pinned `total`.
- The rubric-grader cron must read `pass/total`.

## Alternatives considered

- **`pass/(pass+fail)` with `pending` excluded.** Rejected (see finding 1).
- **Track latest TCK `main` instead of a pinned tag.** Rejected: non-reproducible
  grading; a moving target cannot anchor a GATE. Bumping the pin later is a
  deliberate, recorded action.

## Consequences

Rubric Cat. 4 wording and T-0002 acceptance criteria amended; grader reads the
documented field. Reproducible, non-gameable Cat. 4 metric.

## Resolution (BUG-0007, T+0:54, implementer-wf_84c0f0c7-752-20)

The pin is now concrete and enforced in code.

- **Pinned release tag:** `1.0.0-M23` (openCypher/openCypher).
- **Resolved commit:** `007895aff5f33097d67b2e48a0a2babd6bd18590`.
- **Expected scenario `total`:** **1615** (the Cat. 4 denominator) =
  1339 `Scenario:` + 276 `Scenario Outline:`.
- **Feature-file count:** **220** `.feature` files under `tck/features/`
  (secondary integrity signal).

Why `1.0.0-M23`: it is the last stable milestone of the long-lived `1.0.0-M*`
series (the calendar `2024.x` releases are newer but the M-series is the suite
most widely vendored and referenced by other engines, and it is stable). Bumping
to a newer tag later is a deliberate action: update `caerostris_db::tck`
constants, this section, and re-measure `total` in the same change.

**Provenance of the counts** (measured directly at the pinned commit):

```bash
git clone --depth 1 --branch 1.0.0-M23 \
  https://github.com/opencypher/openCypher.git
cd openCypher
find tck/features -name '*.feature' | wc -l                       # -> 220
grep -rhE '^\s*(Scenario|Scenario Outline):' tck/features \
  --include='*.feature' | wc -l                                   # -> 1615
```

**Enforcement.** The contract lives in `src/tck.rs` (`caerostris_db::tck`):
`PINNED_TCK_TAG`, `PINNED_TCK_COMMIT`, `PINNED_TCK_SCENARIOS`,
`PINNED_TCK_FEATURE_FILES`; `pass_rate(pass, pending, fail) = pass / total`;
`TckSummary` (with `is_complete()` and `to_json()` emitting `tck_tag` +
`tck_commit` + `total`); and `verify_suite_size(loaded)` which errors unless
`loaded == 1615`. The T-0002 harness consumes these rather than computing its own
rate. `Scenario Outline` blocks expand into multiple runtime examples; the pinned
`total` counts *scenarios* (the unit the Gherkin loader yields per `.feature`
file), so `verify_suite_size` is checked against the loaded scenario count, not
the post-expansion example count.
