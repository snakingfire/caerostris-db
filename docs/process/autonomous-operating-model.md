# Autonomous Operating Model — caerostris-db

> How the swarm runs itself for ~4 hours with no human in the loop. Read this
> after [`../commanders-intent.md`](../commanders-intent.md). It defines the
> roles, the cadence, the agile-parallel doctrine, and how you stay accountable
> to the wallclock.

## Doctrine: maximal parallel progress

The system is **agile and concurrent by default**. At any instant there should be
agents simultaneously:

- **specifying** unclear things (steering-gated design),
- **researching** open questions and bringing back options,
- **implementing** the things that are already clearly defined,
- **testing** what's been built and **hunting** for bugs/deviations,
- **proving** the formal properties,
- **sourcing/ingesting** datasets,
- **grading** the project against the rubric and **grooming** the board,
- **curating** docs/memory and **cutting** hourly releases.

You do **not** wait for the whole design to finish before building. Split work:
the clear part ships now; the unclear part is a spike. **Blocking the board is the
cardinal sin.**

## Roles (see `.claude/agents/` for the dispatchable definitions)

| Role | Agent def | Mandate |
|------|-----------|---------|
| Steering committee (×5) | `steering-*` | Ratify designs via adversarial falsification; guard long-term direction & the latency theorem. Opus-tier. |
| Planner / decomposer | `planner-decomposer` | Turn rubric + intent into epics→stories→ready tasks; keep dependencies & readiness current. |
| Researcher | `researcher` | Investigate open questions; return options + recommendation + sources. |
| Dataset scout | `dataset-scout` | Find license-clean graph datasets; plan local-first ingest. |
| Implementer | `implementer` | Build a ready task TDD-first in an isolated worktree; open a simulated PR. |
| Test author | `test-author` | Author unit/integration/property/TCK tests; push coverage to ≥90%. |
| Perf engineer | `perf-engineer` | Criterion benches; validate the latency envelope on the mock. |
| Formal prover | `formal-prover` | TLA+ commit/isolation model; latency cost-model + simulation. |
| Adversarial reviewer | `adversarial-reviewer` | Try to *break* a design or a diff; sign off only when it survives. |
| Pre-mortem analyst | `premortem-analyst` | Assume the change shipped and failed; enumerate how; gate on mitigations. |
| Integrator | `integrator` | Land signed-off PRs onto `main`; keep CI + `format_code.sh` green. |
| Rubric grader | `rubric-grader` | Every 20 min: score, report, file gaps. |
| Pace marshal | `pace-marshal` | Track wallclock vs. plan; relaunch mainspring epochs; raise alarms; write `STOP`. |
| Docs/memory curator | `docs-memory-curator` | Keep CLAUDE.md, ADRs, specs, and agent memory current. |

A single physical agent invocation may wear one hat. Roles are *functions*, run by
many parallel agents.

## Cadence

- **Continuous:** the mainspring loop dispatches waves of ready work (implement →
  review → pre-mortem → land) alongside design/research/test/perf/ingest.
- **Every ~10 min (pace-marshal cron):** groom the board, unblock dependencies,
  relaunch the next mainspring epoch, check pace, alarm if behind.
- **Every 20 min on the wallclock (grader cron):** grade vs. the rubric, commit a
  report to `.project/reports/`, file gap-closing tasks. This is mandatory and
  exact — see [`../requirements/master-rubric.md`](../requirements/master-rubric.md).
- **Every ~60 min (release):** cut an hourly build — see
  [`release-hourlies.md`](release-hourlies.md).

## The work lifecycle (definition of ready / done)

1. **Backlog → Ready** (planner): a task is *ready* when it has clear acceptance
   criteria, named `rubric_refs`, and no unmet `deps`. Design-level tasks must be
   steering-ratified before they are ready to *implement*.
2. **Ready → In progress** (worker claims): set `assignee` + `status`, open a
   worktree, work **TDD-first**.
3. **In progress → In review** (open simulated PR): see
   [`simulated-pr-workflow.md`](simulated-pr-workflow.md).
4. **In review → Done** (land): only after **adversarial review + pre-mortem
   sign-off** and green `format_code.sh` + tests. Integrator lands on `main`.
5. **Design tasks** additionally pass the **design falsification loop** + steering
   sign-off before any dependent implementation is *ready*. See
   [`adversarial-review-loops.md`](adversarial-review-loops.md).

**Definition of done (task):** acceptance criteria met + tests green + coverage not
regressed + reviewer & pre-mortem signed off + docs/ADR updated + board updated.

## WIP limits & throughput

- Favor many **small** tasks over few large ones — small tasks land fast and keep
  the pipeline full.
- Per workflow epoch, concurrency is capped by the harness (~16 concurrent agents);
  the pace-marshal relaunches epochs to sustain throughput across the 4 hours.
- If you finish early, pull the next **highest-rubric-weight** ready task. Idle
  agents are wasted throughput.

## Pace accountability

- Read [`../../.project/pace/deadline.md`](../../.project/pace/deadline.md). Know
  the current T+ marker and the rubric target for it.
- The grader publishes "expected vs. actual" each cycle. If behind, the
  pace-marshal cuts scope toward the highest-weight gates (Cat. 1,2,3,4,7,10,11),
  not toward lower quality of what ships.
- A **GATE** category sliding below its checkpoint target is a P0 alarm.

## Autonomous decision-making

- The responsible agent decides, records the decision (+ alternatives + rationale)
  in `.project/decisions/NNNN-*.md`, and proceeds.
- **Design-level** decisions route through steering (adversarial loop) before they
  bind. **Reversible/local** decisions are made on the spot and logged.
- Conflicts between agents are resolved by the relevant steering member; if
  cross-cutting, by majority of the committee. Record the adjudication.

## Self-improvement (the engine expands itself)

`EPIC-010 — harden the autonomous harness` is P0. The swarm is expected to improve
its *own* machinery: better epoch recycling, dashboards, throughput tuning, sharper
agent prompts. The v1 launcher is the floor. Keep raising it — but never at the
cost of the gates.
