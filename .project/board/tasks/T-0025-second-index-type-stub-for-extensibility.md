---
id: T-0025
title: Stub a second index type against the trait to prove extensibility
type: task
status: blocked
priority: P3
assignee:
epic: EPIC-005
deps: [T-0022]
rubric_refs: [5]
estimate: S
created: T0+0:20
updated: T+5:35
---

## Context

Cat. 5 = 100 requires a second index type stubbed against the same trait to prove
the interface is not leaking B-tree specifics. A range index (or a stub full-text
index) implemented against T-0022's trait demonstrates extensibility without a core
rewrite. Can be done as soon as the trait (T-0022) exists. See `EPIC-005`.

## Acceptance criteria
- [ ] A second index type (range index or stub full-text) implements the T-0022 trait.
- [ ] The implementation required no change to the trait signature (proves the interface generalises) — documented.
- [ ] The planner can consult it through the same trait-based API as the B-tree.
- [ ] tests added (unit covering the second index's trait conformance); coverage not regressed
- [ ] docs / ADR updated noting the extensibility demonstration
- [ ] `./format_code.sh` green

## Notes / log
Ready once T-0022 lands. P3 (extensibility proof) — pull after the B-tree path if
agents are free; cheap and de-risks the Cat. 5 = 100 anchor.

T+4:10 — Two T-0025 implementations exist in flight:
  1. `work/T-0025-second-index-type-stub-for-extensibility` (FullTextIndex) — adversarial-reviewer returned `changes_requested` (T+3:55). Still blocked.
  2. `work/T-0025-second-index-type-stub` (HashIndex) in `.claude/worktrees/wf_e9fceb87-27c-44` — dispatch requested reland, but INTEGRATOR BLOCKED: PR.md review gate checkboxes are BOTH unchecked (adversarial-reviewer and premortem-analyst have NOT signed off). This PR has not been through review. Cannot land without both sign-offs per the non-negotiable landing protocol.

ACTION REQUIRED: The HashIndex implementation (branch `work/T-0025-second-index-type-stub`, worktree `.claude/worktrees/wf_e9fceb87-27c-44`) needs adversarial-reviewer and premortem-analyst sign-offs in its PR.md before the integrator can land it. The implementation looks solid (256 tests, 25 new, format+clippy clean) but the review gate must be cleared first.

T+5:35 (integrator) — Reland attempt for `work/T-0025-second-index-type-stub-for-extensibility` (FullTextIndex) BLOCKED. Dispatch ordered integrator to reland and resolve additive conflict. Integrator investigated and found:
1. Review gate: adversarial-reviewer returned `changes_requested` (T+3:55, PR.md). Both review-gate checkboxes are unchecked (neither adversarial-reviewer nor premortem-analyst have signed approve). Non-negotiable landing protocol: cannot land with open adversarial finding.
2. Blocking adversarial finding: `FullTextIndex` has two inconsistent insertion paths — `index_value()` (which tokenizes "Alice Smith" → "alice", "smith") vs `SecondaryIndex::insert()` (which takes the raw key "alice smith" as one term). Querying via `probe(Equals("Alice Smith"))` after using `index_value` returns empty. The over-claim in ADR 0005 that the full-text index is "consulted identically" through the facade is false-as-written.
3. Rebase: attempted `git rebase origin/main` — first commit hit a conflict in the board file. Aborted. Main has ~20+ commits ahead of the branch; the merge would NOT be purely additive in src/index/mod.rs (main's version has BUG-0019 fixes, new `is_equal_probe_matchable`, `Selectivity::least_selective()` etc. absent from branch).
4. Decision: BLOCKED per non-negotiable. Status remains `in_review`. Author must resolve the adversarial blocking finding, update PR.md with both sign-offs, then re-request integrator reland. The FullTextIndex approach needs one of: (a) model `ContainsTerm` query variant, (b) remove over-claim from ADR/docs and record Cat-5 gap, per adversarial-reviewer resolution options.
