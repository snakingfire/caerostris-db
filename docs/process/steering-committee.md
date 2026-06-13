# Steering Committee — caerostris-db

> The opus-tier design authority for caerostris-db. Five members, each owning
> a domain of the rubric. They guard long-term direction, ratify irreversible
> decisions, and are the final word when the adversarial loop cannot resolve a
> disagreement. Read this alongside
> [`adversarial-review-loops.md`](adversarial-review-loops.md) (when steering
> sign-off is required) and [`autonomous-operating-model.md`](autonomous-operating-model.md)
> (how steering fits into the agile lifecycle).

## Roster

| Handle | Domain | Rubric ownership |
|--------|--------|-----------------|
| `steering-storage` | Storage format, object-store access patterns, GC, manifest layout, range-read design | Cat 2, Cat 9 |
| `steering-query-cypher` | openCypher semantics, query planner, TCK compliance, index selectivity integration, aggregates | Cat 4, Cat 5, Cat 6 |
| `steering-distributed-acid` | Transactions, commit protocol, snapshot isolation guarantee, writer leasing, attach modes, split-brain prevention | Cat 1, Cat 7 |
| `steering-formal-methods` | TLA+/Apalache models, the latency cost-model and simulation, what must be proven before code can be written | Cat 11 |
| `steering-perf-sla` | The latency selectivity-envelope definition, byte budgets, phase-bound K, benchmarks, out-of-envelope detection | Cat 3 |

All five are opus-tier agents (high-stakes, often irreversible decisions;
reasoning quality is non-negotiable). The
[`autonomous-operating-model.md`](autonomous-operating-model.md) lists them
under `.claude/agents/steering-*`.

## Mandate

The committee guards:

1. **Long-term architectural direction** — decisions that are hard or
   impossible to reverse once implementation begins (storage format, commit
   protocol shape, the index trait interface, server-mode wire protocol).
2. **The [GATE] rubric categories** — Cat 1, 2, 3, 4, 7, 10, 11. Any design
   that risks a GATE category falling below 90 requires steering sign-off
   before the corresponding implementation task is marked `ready`.
3. **The latency theorem invariant** from
   [`../commanders-intent.md`](../commanders-intent.md) — the cold-start
   P99 ≤ 1 s target is conditional on the selectivity envelope, and
   `steering-perf-sla` owns the envelope definition. If any design
   implicitly assumes the SLA only holds with cache enabled, or only for
   "lucky" data layouts, that is a falsification and must be escalated
   immediately.

The committee does **not** review every implementation task. Routine tasks that
stay inside a ratified design are reviewed by the adversarial-reviewer /
premortem-analyst loop only. Steering is invoked when:

- A design artifact (spec, ADR, TLA+ model) needs ratification before
  implementation tasks become `ready`.
- An adversarial review loop reaches `reject` verdict or cannot converge.
- Two agents reach conflicting architectural conclusions that require
  adjudication.
- Any agent believes a committed design has been falsified (escalate
  immediately per commanders-intent.md).

## Sign-off protocol

**Which member owns which decision:**

| Decision type | Primary owner | Secondary (consulted) |
|--------------|--------------|----------------------|
| Storage format / manifest layout | `steering-storage` | `steering-perf-sla` |
| Commit protocol / isolation guarantee | `steering-distributed-acid` | `steering-formal-methods` |
| TLA+ model ratification | `steering-formal-methods` | `steering-distributed-acid` |
| Latency envelope parameters (s, B_max, K) | `steering-perf-sla` | `steering-formal-methods` |
| openCypher semantics / TCK scope | `steering-query-cypher` | — |
| Index interface / planner design | `steering-query-cypher` | `steering-storage` |
| Attach modes / writer leasing | `steering-distributed-acid` | `steering-storage` |
| Python binding API surface | `steering-query-cypher` | — |
| Cross-cutting decisions | majority quorum | tie → primary category owner |

**Quorum rule:** decisions that touch two or more members' domains require a
majority (≥ 3 of 5) to ratify. If the vote is 2–2 (with one abstention), the
member who owns the primary affected rubric category casts the deciding vote.
Record the vote tally in the ADR.

**Sign-off is recorded** as an explicit ratification entry in the ADR (see
`docs/adr/README.md`), not as a verbal acknowledgement. The entry must name the
ratifying member(s), the verdict (ratified / ratified-with-conditions /
superseded), and a one-sentence rationale. A design is not ratified until the
ADR record is committed.

## ADR process

Every major or irreversible decision gets an Architecture Decision Record in
`docs/adr/` (see `docs/adr/README.md` for the index and template).

**Lifecycle:**

1. **Proposed** — author opens the ADR draft and files a design-review board
   item (`SPIKE-NNNN`). The draft is submitted to the design falsification loop
   (see [`adversarial-review-loops.md`](adversarial-review-loops.md)).
2. **Adversarially reviewed** — one or more `adversarial-reviewer` agents
   attempt to refute the design; author addresses findings. Loops until the
   design survives.
3. **Ratified** — the relevant steering-committee member(s) sign off (recorded
   in the ADR). The board item for the SPIKE is marked `done`. Dependent
   implementation tasks are now eligible to move from `backlog` to `ready`.
4. **Superseded** — if a ratified ADR must change, a new ADR is opened; the
   old one is marked `superseded: ADR-NNN`. The superseding ADR must explain
   what changed and why.

An implementation task that has an unratified design in its `deps` chain stays
`backlog` until ratification. This is not bureaucracy — it is how we avoid
building the wrong thing at speed.

## Day-one mandate

Before any dependent implementation task moves to `in_progress`, the full
committee must ratify:

1. **Commander's intent** ([`../commanders-intent.md`](../commanders-intent.md))
   — all five members confirm they have read and accept the mission, the latency
   theorem, and the hard constraints.
2. **Master rubric** ([`../requirements/master-rubric.md`](../requirements/master-rubric.md))
   — all five confirm the GATE categories and scoring anchors.
3. **Latency-envelope framing** — `steering-perf-sla` and `steering-formal-methods`
   jointly ratify the envelope parameters (selectivity bound, B_max, K) as a
   first-class spec artifact (`TASK-001`). No storage format or query execution
   implementation task is `ready` until this ADR is ratified.
4. **Commit-protocol approach** — `steering-distributed-acid` and
   `steering-formal-methods` jointly ratify the commit protocol design and the
   TLA+ model scope before the storage implementation (`EPIC-002`) moves past
   skeleton tasks.

These four ratifications are the critical path above all others. The planner
files them as P0 `SPIKE` tasks at T0.
