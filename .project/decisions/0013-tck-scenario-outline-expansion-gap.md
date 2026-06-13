# Decision 0013 — TCK Scenario-Outline expansion gap (named, guarded, deferred)

- **Date / marker:** T0+1:40 (2026-06-13T~20:04:00Z)
- **Owner:** implementer-wf_d44011fb-4d5-6 (executing T-0002)
- **Status:** **RESOLVED** by BUG-0009 (expansion landed). See the resolution
  note at the foot of this file.
- **Rubric:** Cat. 4 (openCypher/TCK), Cat. 10 (tests/coverage)
- **Relates to:** Decision 0008 (pass-rate definition + pinning),
  Decision 0034 (pin reconciliation), BUG-0007, BUG-0018 (Literals6 parse gap —
  previously mis-cited here as "BUG-0008", an unrelated SPDX license bug),
  BUG-0009 (outline expansion).

## Context

The T-0002 TCK harness (`tck-runner/`) parses the vendored openCypher TCK corpus
(pinned tag `2024.3`) with the `gherkin` 0.16 crate and reports
`{ total, pass, pending, fail, parse_errors, pass_rate = pass/total }`.

The `gherkin` 0.16 parser does **not** expand `Scenario Outline:` blocks into one
concrete scenario per `Examples` data row: `feature.scenarios` yields the outline
once, with its `<placeholder>` tokens still literal. The harness therefore counts
each outline **once**.

Empirical composition of the pinned `2024.3` corpus (verified by
`grep`/`tck-runner/tests/vendored_corpus.rs::outline_expansion_gap_is_named_and_guarded`):

| Construct                       | Count |
|---------------------------------|------:|
| plain `Scenario:`               |  1339 |
| `Scenario Outline:`             |   276 |
| **official scenarios** (once each) | **1615** |
| `Examples` data rows            |  2541 |
| **fully-expanded** (plain + example rows) | **3880** |
| BUG-0018 unparseable (`Literals6`) | 13 (in `parse_errors`) |
| **harness `total`** (1615 − 13)  | **1602** |

## Finding (BUG-0009)

Counting outlines once makes the Cat. 4 denominator `1602` rather than the
conventional fully-expanded `~3880`. Two consequences:

1. **Denominator integrity.** Decision 0008 forbids any curated-subset framing of
   the 100% GATE. An unexpanded denominator means ~2541 `Examples` variants are
   never individually executed — latent gaming if it ships undocumented.
2. **False fail / stuck pending under a real engine.** The query handed to the
   engine retains literal `<placeholder>` text; a real engine (EPIC-002) would
   either syntax-error (false `fail`) or report `Unsupported` (permanent
   `pending`), silently capping the achievable pass-rate below 100%.

**Today this corrupts no number.** Under the stub `PendingEngine` every outline
is `pending`, so `0/1602 = 0.0` is internally consistent and honest. The defect
*activates* only when a real engine plugs in.

## Decision

- **Defer outline expansion** to BUG-0009 (P1), to land before/with the first
  real engine in EPIC-002 — expanding outlines now, with no engine to run the
  variants, would only inflate a `pending` denominator without adding signal.
- **Name the gap honestly and guard it now** (parity with how the BUG-0018
  Literals6 parse gap is documented):
  - The vendored-corpus integration test
    `outline_expansion_gap_is_named_and_guarded` pins `PLAIN_SCENARIOS = 1339`,
    `SCENARIO_OUTLINES = 276`, `EXAMPLES_DATA_ROWS = 2541`, the official `1615`,
    the fully-expanded `3880`, and asserts the harness `total` equals the
    **unexpanded** `plain + outlines − unparseable`. The denominator therefore
    cannot silently shift (in either direction) without failing CI.
  - The crate/test module docs state the gap and reference this decision and
    BUG-0009.
- When expansion lands, the guard + the documented denominator are updated
  together — a deliberate, reviewed change, never a silent one.

## Alternatives considered

- **Expand outlines now (manually or via a different parser).** Deferred, not
  rejected: with only the stub engine, expansion adds 2278 more `pending`
  scenarios and zero conformance signal, at the cost of a larger T-0002 diff and
  placeholder-substitution logic that is better co-designed with the real
  executor. Tracked as BUG-0009 so it lands with EPIC-002.
- **Leave it undocumented (status quo of the prior T-0002 attempt).** Rejected:
  that is exactly what got the prior branch returned `changes_requested`; an
  undocumented denominator gap on a GATE is a Decision-0008 violation.
- **Count `pass_rate = pass/(pass+fail)`.** Rejected by Decision 0008 (already).

## Consequences

- T-0002 lands with a real, honest, guarded Cat. 4 metric (`0/1602` today).
- BUG-0009 remains open (P1) to implement expansion before the pass-rate is
  expected to climb (EPIC-002 P1 reads). The grader reads `pass/total` from
  `.project/reports/tck-latest.json` either way.

## Resolution (BUG-0009 — outline expansion landed)

- **Date / marker:** T0+3:05 (2026-06-13).
- **Owner:** test-author (BUG-0009, branch `work/BUG-0009-outline-expansion`).

The harness now expands each `Scenario Outline` into one concrete scenario per
`Examples` data row, substituting `<placeholder>` tokens into the scenario name,
every step value, every docstring (the query + setup statements), and every
data-table cell — implemented in `tck-runner/src/outline.rs::expand_scenario`,
wired through `runner::all_scenarios`. The engine therefore never sees a literal
`<comp>` / `<boolop>`, so the latent "false `fail` / stuck `pending`" defect is
closed.

**Corrected counts (parser-authoritative).** Implementing expansion surfaced a
bug in the *old guard's* row counter: the `grep`-style
`count_gherkin_constructs` helper mis-handled commented-out (`#| ... |`) example
rows in `expressions/precedence/Precedence1.feature`, ending that table early and
dropping **17** real data rows (it reported 36 of the 53 the `gherkin` parser
actually expands). The previously documented `EXAMPLES_DATA_ROWS = 2541` and
fully-expanded `3880` were thus themselves slightly understated. The
authoritative parser figures at tag `2024.3`:

| Quantity                                   | Value |
|--------------------------------------------|------:|
| plain `Scenario:` (whole corpus)           |  1339 |
| plain `Scenario:` (parseable; − Literals6) |  1326 |
| `Scenario Outline:` definitions            |   276 |
| **expanded outline cases** (parser)        |  2558 |
| **harness `total`** (1326 + 2558)          |  **3884** |
| BUG-0018 unparseable (`Literals6`)         | 13 (in `parse_errors`) |

The guard `tck-runner/tests/vendored_corpus.rs::outline_expansion_total_is_reconciled`
now pins the **expanded** denominator (3884), re-derives the composition from the
`gherkin` parser, and additionally asserts that **no `<placeholder>` survives
expansion** in any query/result cell. The denominator still cannot silently shift
in either direction without failing CI (Decision 0008 integrity preserved).
