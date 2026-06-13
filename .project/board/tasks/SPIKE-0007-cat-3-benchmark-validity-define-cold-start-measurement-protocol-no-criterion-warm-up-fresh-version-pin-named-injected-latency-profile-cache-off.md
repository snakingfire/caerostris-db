---
id: SPIKE-0007
title: Cat.3 benchmark validity — define cold-start measurement protocol (no criterion warm-up, fresh version pin, named injected-latency profile, cache off)
type: spike
status: done
priority: P0
assignee: researcher
epic: EPIC-003
deps: []
rubric_refs: [3, 10]
estimate: S
created: 2026-06-13T18:30:45Z
updated: 2026-06-13T19:10:00Z
---

## Context

Filed by `steering-perf-sla` during the ratification pass of `docs/commanders-intent.md`
and `docs/requirements/master-rubric.md`. See decision
`.project/decisions/0002-perf-sla-ratification-pass.md`.

The graded documents require "cold-start P99 ≤ 1 s ... without the cache", but **neither
the rubric (Cat. 3) nor commander's-intent defines what a valid cold-start measurement is.**
The only place the measurement protocol lives is `docs/process/testing-and-benchmarks.md`
(§5, §7) — and that doc has a **direct confound**: criterion is described as running with a
"default: 10-sample warm-up". Criterion's warm-up is the opposite of a cold start. If the
rubric-grader accepts a standard `cargo bench` P99 as Cat. 3 evidence, it will be scoring a
**warm-process, warm-OS-page-cache, warm-version-pin** number against a cold-start SLA. That
is exactly the "fast only when warm" falsification the commander's intent forbids — except
it would slip in through the *measurement*, not the design.

A second gap: the SLA target is stated as "P99 ≤ 1 s" but the **injected S3 latency profile
is not named in the graded docs**. "P99 ≤ 1 s on the mock" is meaningless without stating the
injected per-request latency; testing-and-benchmarks defines `fast-s3` (5 ms) / `nominal-s3`
(20 ms) / `slow-s3` (50 ms) but the rubric does not bind Cat. 3 acceptance to any of them.
A green Cat. 3 obtained under `loopback` (0 ms) or `fast-s3` (5 ms) is not evidence the real-S3
SLA holds.

This spike defines the **cold-start latency benchmark protocol** as a first-class, grader-
readable artifact so the weight-14 GATE cannot be scored on invalid evidence.

## Acceptance criteria

- [x] A documented cold-start measurement protocol (ADR or `docs/design/`) that specifies:
  - Each timed sample is a **fresh engine/process or explicitly evicted state**: no warm
    OS page cache, no warm local cache, a fresh manifest/version pin per sample. Criterion's
    warm-up must be disabled or the harness must re-cold between samples (or a bespoke
    sampler used instead of criterion's default loop).
  - The **local cache is explicitly OFF** for the cold-start run, and there is a separate
    CI-enforced test asserting the SLA holds with cache disabled (satisfies R9 / Cat. 9 100-anchor).
  - The **injected-latency profile is named** in every recorded result
    (`latency_profile: nominal-s3` etc.) and the **acceptance bar is pinned to a profile**
    (target = `nominal-s3` 20 ms, ceiling = `slow-s3` 50 ms — to be ratified by steering).
  - **N (sample count) and the P99 estimator** are stated; N ≥ 200 required; P99 = sample at ⌈0.99×N⌉.
- [x] The rubric-grader's Cat. 3 evidence rule is updated (or a note filed for the grader)
      so that a warm/loopback criterion number is **not** accepted as cold-start evidence.
      Normative filter defined in ADR 0001.
- [x] `.project/reports/benchmark-history.jsonl` schema includes `cold: true|false`,
      `cache: on|off`, `latency_profile`, and `samples` so evidence is self-describing.
      Schema specified in ADR 0001 Rule 5.
- [x] Cross-referenced from EPIC-003; consistent with SPIKE-0006 (the K·L_p99 floor sets
      what a *plausible* cold P99 even looks like under each profile).
- [x] docs updated (`testing-and-benchmarks.md` §7 amended); `./format_code.sh` green (no code touched).

## Notes / log

- T0+0:15 — filed by steering-perf-sla. The confound is real and load-bearing: it is the
  most likely way the latency theorem gets *measured* as passing while actually failing cold.
  Does not block launch — it blocks a valid Cat. 3 = 100 sign-off. Owner: perf-engineer +
  steering-perf-sla, with grader-input coordination (T-0005 / rubric-grader).
- T0+~0:41 — **DONE** by researcher. Committed `docs/adr/0001-cold-start-benchmark-protocol.md`
  (ADR defining the five normative rules). Updated `docs/process/testing-and-benchmarks.md` §7
  to cross-reference the ADR and add the normative cold-start protocol summary and acceptance-bar
  table. Acceptance criteria status:
  - [x] Documented cold-start protocol: fresh state per sample, cache explicitly OFF,
        latency profile named, N ≥ 200 with stated P99 estimator — see ADR 0001 Rules 1–4.
  - [x] Rubric-grader Cat. 3 evidence rule: normative filter defined in ADR 0001
        ("Grader evidence rule" section); `loopback`/`fast-s3` explicitly excluded.
  - [x] `benchmark-history.jsonl` schema: ADR 0001 Rule 5 specifies the five required fields
        (`cold`, `cache`, `latency_profile`, `samples`, `passed`).
  - [x] Cross-referenced from EPIC-003 and consistent with SPIKE-0006 (acceptance bars at
        `nominal-s3` 20 ms and `slow-s3` 50 ms align with the K_min·L_p99 floor analysis).
  - [x] docs updated (testing-and-benchmarks.md §7); no code touched so format_code.sh n/a.
  Landing commit: board + ADR commit following this entry.
  Follow-up: steering-perf-sla must ratify ADR 0001 before the Cat. 3 bench harness is built.
  File a task for the perf-engineer to implement the bench harness (iter_custom / bespoke
  sampler) once EPIC-008 provides the `cache.enabled` config key.
