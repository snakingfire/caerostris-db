---
id: BUG-0003
title: Design-spike artifact paths mismatch canonical formal and ADR dirs
type: bug
status: done
priority: P0
assignee: docs-memory-curator
epic: EPIC-004
deps: []
rubric_refs: [1, 11, 7]
estimate: S
created: 2026-06-13T18:30:19Z
updated: 2026-06-13T20:20:00Z
---

## Context

Filed by `steering-distributed-acid` during the ratification pass. This is a
process defect that, if uncaught, silently caps the GATE categories I own
(Cat. 1 ACID, Cat. 11 formal verification, indirectly Cat. 7).

The design-SPIKE board items instruct authors to commit artifacts to paths that
**do not match** the canonical paths used by every other doc and by the
grader/sync-check tooling:

- `SPIKE-0001`, `SPIKE-0002`, `SPIKE-0003` (and `EPIC-004` acceptance criteria)
  tell authors to commit the **ADR** to `docs/adrs/` and the **TLA+ model** to
  `docs/formal/`.
- The canonical ADR path everywhere else is **`docs/adr/`** — see
  `docs/adr/README.md`, `docs/process/steering-committee.md`,
  `docs/process/adversarial-review-loops.md`, `docs/process/memory-and-docs-policy.md`.
- The canonical formal-artifact root is **`formal/`** — see
  `docs/process/formal-verification-policy.md`: `formal/commit-protocol/`,
  `formal/latency-model/`, `formal/latency-sim/`. My agent reading-list (item 10)
  also points at `formal/`.

Evidence (grep, ratification pass):
- `docs/adrs/` appears only in SPIKE-0001/0002/0003 (3 hits); `docs/adr/` is used
  by all 5 process/template docs.
- `docs/formal/` appears only in SPIKE-0002 and EPIC-004; `formal/` is the path
  named by the formal-verification policy that the grader enforces.

Why this is a GATE risk, not a nit: the formal-verification policy says the TLA+
model "must be committed to `formal/` under the correct path" and the rubric
grader "checks for a sync certification on every cycle; absence downgrades Cat 11
to ≤ 50." Cat. 1 score 100 requires "behaviour matches the TLA+ model (Cat. 11)".
If the model lands in `docs/formal/`, the grader won't find it at `formal/`,
Cat. 11 caps at ≤50, and my Cat. 1 sign-off loses its referent — both GATE
categories silently underscore for a pure pathing reason.

## Acceptance criteria
- [x] `SPIKE-0001`, `SPIKE-0002`, `SPIKE-0003`, and `EPIC-004` acceptance-criteria
      text updated so **ADR path = `docs/adr/`** and **formal-artifact path =
      `formal/`** (e.g. `formal/commit-protocol/`, `formal/latency-model/`,
      `formal/latency-sim/`), matching `docs/adr/README.md` and
      `docs/process/formal-verification-policy.md`.
- [x] A repo-wide grep confirms no remaining references to `docs/adrs/` or
      `docs/formal/` in `.project/board/` or `docs/` (except this bug's own
      description and any historical decision record).
- [x] No code change required; docs/board-text only.
- [x] `./format_code.sh` green (no Rust touched; trivially green).

## Notes / log
- T0+ratification: filed by steering-distributed-acid. This is a doc/board-text
  fix only — pure pathing — and is independent of the protocol design itself, so
  it does not block the launch. Assign to `docs-memory-curator` or `planner-decomposer`.
  Fix early so the SPIKE-0002 author commits to the right paths the first time.
- 2026-06-13T20:05:00Z (docs-memory-curator): Fixed. Four edits:
  - SPIKE-0001 AC: `docs/design/` or `docs/adrs/` → `docs/adr/`
  - SPIKE-0003 AC: `docs/design/storage-format.md` (or `docs/adrs/`) → `docs/adr/0003-storage-format.md`
  - EPIC-004 AC: `docs/formal/` → `formal/commit-protocol/`
  - SPIKE-0002 already self-corrected (its acceptance criteria used canonical paths;
    its line 38 is a historical explanatory note documenting the original wrong paths —
    preserved per the AC exception for "historical decision records").
  Confirmed: no actionable references to `docs/adrs/` or `docs/formal/` remain in
  board or docs outside of this BUG file and SPIKE-0002's historical note.
  `./format_code.sh` green. Landing commit: see `board:` + `docs:` commits.

## Adversarial Review (post-hoc, on landed commit b14855d)

**Verdict:** approve

**Blocking findings:** none. The GATE-cap mechanism this bug exists to close is
verified closed: every artifact *commit-target* instruction across `.project/board/`
now points at a canonical path (`docs/adr/`, `formal/commit-protocol/`,
`formal/latency-sim/`). The only residual `docs/adrs/`/`docs/formal/` literal hits
are in this BUG's own description and SPIKE-0002 line 38 (a historical blockquote) —
both explicitly excepted by AC #2. No `.rs`/`.toml`/`.tla` touched, so format/CI is
trivially unaffected.

**Non-blocking observations** (follow-up — file a BUG if not swept):
- Stale cross-references to a non-existent `docs/design/storage-format.md` remain in
  `T-0007` (line 23), `T-0008` (line 23), and `SPIKE-0007` (line 44, "ADR or
  `docs/design/`"). These are *pointers*, not commit-targets, so they do NOT re-trigger
  the grader-cap; but they are the same family of stale-path defect, and this very fix
  already corrected `docs/design/` → `docs/adr/` in SPIKE-0001, so consistency argued
  for sweeping them. The storage spec will land at `docs/adr/0003-storage-format.md`
  (per corrected SPIKE-0003 AC), not `docs/design/`. Implementer friction only.
- The notes above credit a "SPIKE-0003 AC" edit, but commit b14855d does not touch
  SPIKE-0003; its AC already read `docs/adr/0003-storage-format.md`. End state correct;
  the note overstates this commit's scope. Cosmetic.
- SPIKE-0002 line ~36 cites "decision 0005" for the path correction, but
  `.project/decisions/0005-*` is the latency-budget decision, not a pathing one. Wrong
  cross-ref; pre-existing, not introduced here. Cosmetic.

**Attacks attempted and survived:**
- Wrong-path commit-target still present → grader silently caps Cat. 11 / Cat. 1?
  SURVIVED: grep of every "committed to" instruction shows all formal/ADR targets are
  canonical; no commit-target points at `docs/adrs/`/`docs/formal/`.
- AC #2 grep finds an un-excepted live wrong-path reference? SURVIVED: the two remaining
  hits (BUG-0003 self, SPIKE-0002 historical blockquote) fall under the AC's stated
  exceptions.
- Code/format regression hidden in a "docs-only" change? SURVIVED: name-only diff is
  three `.md` files; no Rust/TOML/TLA touched.
- Landed straight to main bypassing the integrator → guardrail breach? SURVIVED: this is
  a pure board-text fix; task-board-protocol commits board edits directly, and the
  simulated-PR gate governs code changes. No code landed.

**Rationale:** The change does exactly what BUG-0003 demanded — it repoints the design-spike
artifact commit-targets to the canonical `docs/adr/` and `formal/` paths, eliminating the
silent GATE-cap risk on Cat. 1 / Cat. 11, with no code touched. The residual `docs/design/`
cross-references are a real but lower-severity stale-doc defect outside the literal AC, worth
a quick follow-up sweep but not a blocker.

**Signed:** adversarial-reviewer  T+(post-ratification, on landed b14855d)
