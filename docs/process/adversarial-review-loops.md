# Adversarial Review Loops — caerostris-db

> Two mandatory quality loops. Neither is optional. Neither is a formality.
> "Looks fine" is not a sign-off. Default posture is **skepticism**; the burden
> of proof is on the author, not the reviewer. Read this alongside
> [`autonomous-operating-model.md`](autonomous-operating-model.md) (when these
> loops fire) and [`simulated-pr-workflow.md`](simulated-pr-workflow.md) (where
> verdicts are recorded).

## Loop A — Design falsification loop

Fires on: any proposed design, spec, ADR, or formal model before the corresponding
implementation tasks are marked `ready`. This includes latency-envelope proofs,
commit-protocol specs, openCypher semantics decisions, index interface designs —
anything that binds future implementation.

### Process

1. **Author** commits the design artifact (spec, ADR draft, TLA+ model) and
   opens a review request on the board item (`status: in_review`).

2. **One or more `adversarial-reviewer` agents** are dispatched with an
   explicit mandate: **find a case where this design breaks**. Specifically:
   - Does the latency theorem hold under the stated selectivity/byte-budget
     assumptions, or is there a query shape inside the claimed envelope that
     busts the SLA?
   - Is the ACID guarantee provable, or does the commit protocol have a
     linearizability gap under crash + concurrent reader?
   - Does the interface satisfy all callers, or is there a caller whose
     contract this design cannot meet without breaking another invariant?

   Adversarial reviewers are not asked for "suggestions" — they are asked to
   **refute**. If they cannot refute, that is the finding.

3. **Author addresses or rebuts** each blocking finding. A rebuttal must be
   reasoned (cite the model, the spec, or a proof sketch); "I disagree" is not
   a rebuttal. Acceptable outcomes per finding: (a) design updated to fix the
   gap, or (b) written rebuttal explaining why the finding does not apply,
   accepted by the reviewer.

4. **Loop until:** the design survives all adversarial rounds (no open blocking
   findings) **and** the relevant
   [steering-committee](steering-committee.md) member(s) have signed off.
   Steering sign-off is required in addition to reviewer sign-off — they are
   not the same gate.

5. **Steering sign-off records** an explicit ratification entry in the ADR
   (see `docs/adr/README.md`). The board item's `deps` on the design task clear
   only after ratification.

### Rounds

- Expect 1–3 rounds for typical design artifacts.
- There is no round cap. If a design cannot survive adversarial review, it is
  not ready to implement — escalate to the steering committee to adjudicate or
  redesign.
- Rounds are fast (minutes, not hours). Do not treat "another round" as a
  failure; treat an unreviewed design as a failure.

---

## Loop B — Code review + pre-mortem loop

Fires on: every diff that touches the implementation (any change to `.rs`
files, `Cargo.toml`, the formal models, or test suites) before the integrator
lands it.

### Sub-loop B1: adversarial code review

An `adversarial-reviewer` agent is given the full diff and the acceptance
criteria from the board item. Its job is to **try to break it**:

- **Correctness:** is there a code path, edge case, or concurrent interleaving
  that violates the acceptance criteria or an invariant from the rubric (e.g.
  snapshot isolation, latency envelope, format atomicity)?
- **Security / open-source guardrails:** does this introduce a secret, an
  unsafe block without justification, a non-license-clean dependency, or a
  data leak?
- **Simplicity / blast radius:** is there a simpler implementation that
  achieves the same guarantee? Does this change add accidental complexity that
  will cost us later?

The reviewer does **not** suggest style improvements or nitpick names unless
they affect correctness. Signal-to-noise matters — findings must be actionable.

### Sub-loop B2: pre-mortem

A `premortem-analyst` agent is told: *"This change has just shipped to
production and caused an incident. Enumerate the most likely failure modes."*
It does not evaluate whether the code is correct — it assumes it shipped and
asks what went wrong. Common pre-mortem angles:

- Race condition or crash window the tests do not exercise.
- Latency regression outside the envelope that only surfaces at scale.
- Manifest corruption or GC gap that only shows up after a write burst.
- A Python binding edge case that fails silently.
- A format-version migration path that wasn't tested.

For each failure mode: (a) is it plausible given the diff? (b) is there a
mitigation already in the change? (c) if not, what must be added before landing?

Findings that lack a mitigation are **blocking**.

### Verdict schema

Both reviewer roles emit a structured verdict appended to `PR.md`:

```
## Adversarial-reviewer verdict  (or: Pre-mortem verdict)
verdict: approve | changes_requested | reject
blocking_findings:
  - <id>: <one-line description>
    detail: <explanation; cite the code path or model that breaks>
    mitigation_required: <what must change before landing>
non_blocking_notes:
  - <optional; things the author should know but need not fix now>
rationale: >
  <1–3 sentences explaining the overall assessment>
```

`approve` = no blocking findings; the reviewer would merge this.
`changes_requested` = blocking findings exist; author must address before
re-review.
`reject` = the approach is fundamentally flawed; a rework (not a patch) is
needed; escalate to the steering committee.

**"Looks fine" is not a rationale.** A rationale must state what the reviewer
checked and why the design/code holds under those checks.

### Process

1. Worker opens the simulated PR (see [`simulated-pr-workflow.md`](simulated-pr-workflow.md)).
2. `adversarial-reviewer` and `premortem-analyst` are dispatched concurrently.
3. Author addresses all blocking findings; appends responses inline below each
   finding in `PR.md`.
4. Reviewer re-reviews the response; updates verdict. One re-review round is
   standard. If a third round is needed, the finding is escalated to the
   steering committee for adjudication.
5. Both verdicts must be `approve` before the integrator is called.

### What sign-off means

Sign-off is the reviewer appending their verdict block with `verdict: approve`
to `PR.md` and the board item's `in_review` record. It is not a Slack message.
It is not implied by silence. It is a committed, explicit record that a
responsible agent checked this change and found no blocking reason not to land
it. That record travels with the branch into `main` forever.
