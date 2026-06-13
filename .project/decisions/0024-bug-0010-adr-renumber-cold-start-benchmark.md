# Decision 0024 — BUG-0010: renumber the cold-start-benchmark-protocol ADR to 0004

- **Date:** 2026-06-13
- **Author / role:** `implementer-wf_f36e3f02` (BUG-0010)
- **Type:** reversible, local docs-hygiene decision (file rename + link updates)
- **Rubric:** Cat. 12 (engineering & process health)
- **Board item:** `BUG-0010`
- **Supersedes / relates:** finding F3 of decision `0015` (the SPIKE-0001
  ratification that filed this collision).

## Decision

Resolve the `docs/adr/0001` numbering collision (BUG-0010) by renumbering
`docs/adr/0001-cold-start-benchmark-protocol.md` to
**`docs/adr/0004-cold-start-benchmark-protocol.md`**, leaving the canonical,
widely-referenced `0001-latency-selectivity-envelope.md` at `0001`.

## Rationale

Two ADRs shared number `0001` — the latency-selectivity-envelope ADR (SPIKE-0001,
`accepted`) and the cold-start-benchmark-protocol ADR (SPIKE-0007, `proposed`).
This breaks the ADR README's unique-number rule, ambiguates cross-references, and
risks the rubric-grader mis-attributing Cat. 3 / Cat. 10 evidence.

- **Which ADR moves:** the benchmark-protocol ADR has the fewest *live* inbound
  links — exactly two, both markdown links in
  `docs/process/testing-and-benchmarks.md`. The envelope ADR is referenced across
  decisions, reports, and many board items, so moving it would be far higher
  churn. The BUG-0010 board item explicitly directs: "Do NOT renumber
  `0001-latency-selectivity-envelope.md`."
- **Which number:** `0002` is **not** free — it is reserved for the in-flight
  SPIKE-0002 S3 commit-protocol ADR (`docs/adr/0002-s3-commit-protocol.md`,
  produced on the unlanded `work/SPIKE-0002-…` branch and already referenced by
  decisions `0012`, `0014`, and `0023`). Taking `0002` would re-create a collision
  the moment SPIKE-0002 lands. `0003` is taken by the landed
  server-mode-network-protocol ADR. The next genuinely free ADR number is
  **`0004`** (no `docs/adr/0004-*` file or markdown link exists anywhere on
  `main`).

## What changed

- `git mv docs/adr/0001-cold-start-benchmark-protocol.md
  docs/adr/0004-cold-start-benchmark-protocol.md` (history preserved).
- Updated the ADR's own title heading: `# ADR 0001 …` → `# ADR 0004 …`.
- Updated the two live markdown links in `docs/process/testing-and-benchmarks.md`.
- Marked finding F3 **RESOLVED** in `docs/adr/0001-latency-selectivity-envelope.md`
  (the forward-looking instruction text; the historical "what was found" sentence
  is left intact, reworded only past-tense).
- Updated `docs/adr/README.md` Naming section to mandate uniqueness, warn about
  reserved-by-unlanded-branch numbers, and point at the new regression guards.
- Added two guards in `tests/repo_hygiene.rs`: `adr_numbers_are_unique` (fails on a
  duplicate ADR sequence number) and `adr_markdown_links_are_not_dangling` (fails
  on a `docs/adr/NNNN-…md` link with no target file).
- Appended an append-only forward-pointer note to the SPIKE-0007 board task log.

Append-only historical records were intentionally **left unchanged**: decision
`0015` (the ratification that filed F3), the frozen rubric report
`.project/reports/rubric-T+00-37.md`, and decision `0017`. Those record state at a
point in time and must not be rewritten per `docs/process/memory-and-docs-policy.md`.

## Alternatives rejected

- **Renumber the envelope ADR instead:** rejected — it breaks 6+ inbound
  references across decisions, reports, and board items; far higher churn and risk.
- **Use `0002` as "the next free number":** rejected — `0002` is reserved for the
  in-flight commit-protocol ADR; reusing it just relocates the collision.
- **Rewrite every historical mention to the new number:** rejected — decision logs
  and frozen reports are append-only/immutable; a forward-pointer note is the
  correct, policy-compliant update.

## Out of scope (noted for follow-up)

- `.project/decisions/` itself contains pre-existing duplicate numbers — e.g. four
  `0012-*` files and two `0017-*` files. That is a separate decision-log hygiene
  defect in a different namespace; BUG-0010 is scoped to `docs/adr/`. Recommend the
  planner / docs-curator file a follow-up BUG. (This decision uses `0024` to avoid
  adding to that mess.)
