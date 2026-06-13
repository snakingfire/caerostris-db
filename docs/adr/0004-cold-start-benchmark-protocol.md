# ADR 0004 — Cold-Start Benchmark Protocol for Cat. 3 Latency Validation

## Status

`proposed`

> **Renumbered 2026-06-13 (T0+~3:05) by docs-memory-curator (BUG-0010).** This ADR
> was originally filed as `0001` and collided with the canonical
> `0001-latency-selectivity-envelope.md`. It was renumbered to the next free ADR
> sequence number (`0004`) — the lower-churn fix, since the envelope ADR has far
> more inbound references. The decision content is unchanged. Inbound references
> in living docs were updated in the same change; append-only decision logs and
> rubric reports retain their historical `0001-cold-start-benchmark-protocol`
> mentions by design (they record what was true at the time).

## Date / T+ marker

2026-06-13T19:05:00Z (T0+~0:41); renumbered 0001→0004 at T0+~3:05 (BUG-0010)

## Context

Filed to resolve the measurement confound identified by `steering-perf-sla` in
`.project/decisions/0010-perf-sla-ratification-pass.md` (finding 3) and tracked
as `SPIKE-0007`.

The master rubric (Cat. 3, weight 14, **[GATE]**) requires:

> "benchmark on the mock (injected latency) meets it; out-of-envelope handling
> implemented and tested; **no reliance on cache**."

The commander's intent also states:

> "The cold-start SLA must hold **without** the local cache."

Neither document defines *what constitutes a valid cold-start measurement*. The
only existing protocol is in `docs/process/testing-and-benchmarks.md` §5, which
describes standard criterion usage with a **"default: 10-sample warm-up"** — the
opposite of a cold start. If the rubric-grader accepts a standard `cargo bench`
P99 as Cat. 3 evidence, it would score a warm-process / warm-OS-page-cache /
warm-version-pin number against a cold-start SLA. This is precisely the
"fast only when warm" falsification that the commander's intent forbids — except
it would slip in through the *measurement*, not the design.

A second gap: the SLA is stated as "P99 ≤ 1 s on the mock" but the injected S3
latency profile is unnamed in the graded documents. A green Cat. 3 obtained under
`loopback` (0 ms) or `fast-s3` (5 ms) is not evidence the real-S3 SLA holds.
`docs/process/testing-and-benchmarks.md` §7 defines four latency profiles but
does not bind the Cat. 3 acceptance bar to any one of them.

A third gap: "P99 of < 100 samples" is not a statistically meaningful P99
estimate. The sample count and the P99 estimator must be stated explicitly.

### Rubric categories affected

- **Cat. 3** (Latency: selectivity-envelope theorem + measured SLA) — GATE, w14.
  The protocol defined here is the sole mechanism by which the grader may accept
  a measured result as cold-start evidence. Without this ADR, Cat. 3 cannot reach
  100 on valid evidence.
- **Cat. 9** (Caching, w4) — the 100-anchor requires "a test proving the cold SLA
  holds with caching disabled". The protocol here is the shared measurement basis.
- **Cat. 10** (Tests/coverage/benches, w8) — GATE. Criterion bench hygiene and the
  `.project/reports/benchmark-history.jsonl` schema are both in scope.

### Prior decisions this builds on

- `.project/decisions/0010-perf-sla-ratification-pass.md` — steering-perf-sla
  ratification pass; this ADR implements its finding 3 directly.
- `docs/process/testing-and-benchmarks.md` §7 — defines the four latency profiles;
  this ADR binds the acceptance bar to specific profiles and adds cold-start
  constraints not yet present there.
- `SPIKE-0006` — establishes the K_min·L_p99 latency floor under each profile;
  that floor is the analytical basis for which profiles are *plausible* targets.

---

## Decision

We will define a **cold-start benchmark protocol** as the only measurement method
whose output the rubric-grader may accept as Cat. 3 evidence. The protocol has
five normative rules.

### Rule 1: State isolation between samples

Each timed sample must begin from a fully cold state:

1. **No OS page cache.** The process must either:
   - be a freshly spawned child process with a separate address space (preferred),
     **or**
   - call `sync; echo 3 | sudo tee /proc/sys/vm/drop_caches` between samples when
     running as root in a controlled CI environment (Linux only).

   On macOS/developer machines: use the fresh-process approach exclusively;
   `purge` requires root and is unreliable in CI.

2. **No warm local cache.** The engine cache must be explicitly disabled via the
   configuration flag `cache.enabled = false` (the config key that will be
   established by the Cat. 9 implementation). The benchmark binary must assert at
   startup that the cache is off and abort if it is not.

3. **Fresh manifest/version pin per sample.** The engine must open the database
   from scratch for each sample — no persistent `Database` handle across samples.
   Criterion's default benchmark loop reuses state across iterations; this violates
   Rule 1.3. See Rule 2 for the harness consequence.

### Rule 2: Bespoke cold-start sampler, not criterion's default loop

Criterion's `Bencher::iter()` calls the closure many times in a tight loop,
performing a warm-up phase and then timed iterations without any state-reset
between them. This is correct for throughput benchmarks but invalid for
cold-start latency benchmarks.

The **cold-start latency bench** at `benches/query_6hop.rs` (and any other Cat. 3
bench) must use **criterion's `bench_function` with `Bencher::iter_custom()`**,
providing a closure that:

1. Spawns a fresh child process (or, as a fallback, a fresh `tokio` runtime +
   engine open), performs the query, records the elapsed time, and returns it.
2. Explicitly sets `cache.enabled = false` in the engine config passed to each
   iteration.
3. Drops the engine handle fully before recording the time (timing includes the
   full open-query-close cycle, because cold-start latency *is* the open-to-result
   latency).

Criterion's warm-up machinery (`Criterion::warm_up_time`) must be set to zero
(`Duration::from_secs(0)`), or the bench must use a custom iterator that does not
call the warm-up path. A comment in the bench source must explain why warm-up is
disabled.

Alternative: use a completely bespoke sampling loop outside of criterion (see
Alternative B below) and write the results directly to `benchmark-history.jsonl`.
This is acceptable if criterion's API makes Rule 2 difficult to satisfy cleanly.

### Rule 3: Named injected-latency profile; pinned acceptance bars

Every recorded benchmark result must carry a `latency_profile` field identifying
which injected-latency scenario was used (see §7 of `testing-and-benchmarks.md`
for the profile definitions). The Cat. 3 acceptance bars are:

| SLA tier | Profile | Per-request latency | Jitter | Required outcome |
|---|---|---|---|---|
| **Target** | `nominal-s3` | 20 ms | 5 ms | P99 ≤ 1 s |
| **Hard ceiling** | `slow-s3` | 50 ms | 10 ms | P99 ≤ 2 s |

Rationale: `nominal-s3` (20 ms) corresponds to typical cross-AZ S3 and is the
deployment environment the design is primarily optimized for. `slow-s3` (50 ms)
is the conservative / degraded scenario that the conditional theorem must also
survive (at L_p99 = 50 ms, K_min = 8 gives a floor of 400 ms, well inside 1 s;
K_min = 8 gives 400 ms floor under slow-s3, leaving 600 ms for data transfer and
compute — tight but feasible for in-envelope queries). This matches the
recommendation in decision `0010`.

A Cat. 3 result recorded under `loopback` (0 ms) or `fast-s3` (5 ms) **may not**
be cited as evidence for the P99 ≤ 1 s cold-start SLA. It may be cited as
evidence for raw engine throughput (loopback) or for a fast-region bonus target.
The rubric-grader must reject `latency_profile: "loopback"` or
`latency_profile: "fast-s3"` as Cat. 3 primary evidence.

### Rule 4: Minimum sample count and P99 estimator

A statistically meaningful P99 estimate requires:

- **N ≥ 200 samples** for the Cat. 3 primary measurement (cold-start, cache off,
  nominal-s3 or slow-s3 profile). This is the minimum to estimate the 99th
  percentile with an expected absolute error of ≤ 1–2 samples at the tail.
- **P99 estimator:** the 199th-order statistic of 200 sorted samples (i.e., the
  maximum of the top two) or, for larger N, the sample at the ⌈0.99 × N⌉-th
  position. No interpolation is required.
- The recorded result must include `samples: N` alongside the P99 value so the
  grader can verify Rule 4 compliance.

Rationale: with N = 100, the 99th percentile is estimated by a single sample —
the maximum — which has high variance and no neighbor for validation. 200 samples
give two tail samples, providing a minimal sanity check. For the cold-start path
(each sample spawns a process and runs a full open-query-close cycle) 200 samples
takes roughly 200 × P50 wall-clock time; at a P50 of ~500 ms this is ~100 s of
bench time, which is acceptable in a dedicated CI bench job.

### Rule 5: Self-describing result schema for `benchmark-history.jsonl`

Every entry appended to `.project/reports/benchmark-history.jsonl` for a Cat. 3
(cold-start) measurement must include these fields alongside the existing ones:

```jsonc
{
  "benchmark": "query_6hop",           // bench name
  "git_sha": "<sha>",                   // git SHA of the build
  "timestamp": "<ISO8601>",            // wall-clock timestamp
  "cold": true,                        // REQUIRED: true = cold-start protocol applied
  "cache": "off",                      // REQUIRED: "off" | "on"
  "latency_profile": "nominal-s3",     // REQUIRED: one of loopback|fast-s3|nominal-s3|slow-s3
  "samples": 200,                      // REQUIRED: number of cold samples taken
  "p99_ms": 843.2,                     // P99 latency in milliseconds
  "p50_ms": 510.1,                     // P50 latency in milliseconds
  "sla_target_ms": 1000,               // the SLA target being tested against
  "sla_ceiling_ms": 2000,              // the hard ceiling
  "passed": true                       // p99_ms <= sla_target_ms
}
```

Any entry with `cold: false` or `cache: "on"` may be used for warm / cached
tracking but **must not** be cited as Cat. 3 primary evidence. The grader must
filter on `cold: true AND cache: "off" AND latency_profile IN ("nominal-s3",
"slow-s3")` when reading Cat. 3 evidence.

---

## Alternatives considered

### Alternative A — Use criterion's warm-up with 0 samples, rely on `--measurement-time` only

**Description:** Configure criterion with `warm_up_time(Duration::from_secs(0))`
and set a very long `measurement_time`. Between iterations, manually drop the
engine handle. Accept that the OS page cache and process state may carry over
between iterations (since the same process persists across the bench loop).

**Why considered:** Minimal code change from the existing `cargo bench` workflow.
No need for a bespoke sampler.

**Why rejected:** The OS page cache is not evicted between iterations even with
0-sample warm-up. The engine's internal state (Tokio runtime, thread pool, open
file descriptors) persists across iterations. A "0 warm-up" criterion bench on the
*second and subsequent* iterations still benefits from warm kernel caches. This
violates Rule 1.1 and Rule 1.3, producing results that measure warm-cache
performance while labelling it cold-start. This is precisely the confound that
SPIKE-0007 was filed to close.

### Alternative B — Completely bespoke sampler (no criterion)

**Description:** Write a standalone binary or a `#[test]`-annotated function that
spawns N child processes, records each child's elapsed time, sorts the results, and
computes the P99. Append directly to `benchmark-history.jsonl`. No criterion
dependency for this bench.

**Why considered:** Gives full control over state isolation; no criterion API
constraints to work around. Matches the "fresh process per sample" requirement
exactly.

**Why rejected (as the primary approach):** Criterion provides statistical summary
output, HTML reports, and baseline comparison that are useful for tracking
regressions across the run. Abandoning criterion entirely loses this tooling. The
recommended approach (Rule 2, `iter_custom`) retains criterion as the harness
while satisfying the cold-start requirement. However, Alternative B is explicitly
**permitted as a fallback** if `iter_custom` proves insufficient (e.g., if
criterion's warm-up path cannot be cleanly suppressed via the public API). In that
case the bench binary writes its own JSONL output conforming to Rule 5, and
criterion is not used for the cold-start bench.

### Alternative C — Accept loopback or fast-s3 as a valid Cat. 3 profile

**Description:** Bind the Cat. 3 acceptance bar to `loopback` (0 ms) or `fast-s3`
(5 ms), which are easy to achieve and already described in `testing-and-benchmarks.md`.

**Why considered:** Simpler to pass. Less wall-clock bench time.

**Why rejected:** A result under 0 ms or 5 ms injected latency tells us nothing
about whether the design survives real S3 at 20–50 ms per request. The purpose of
the Cat. 3 gate is to provide evidence that the conditional theorem holds in a
realistic deployment. A fast-loopback result is evidence only of raw compute speed.
Accepting it as the Cat. 3 primary evidence bar would be the exact "fast only with
luck / fast only on the mock" failure mode the commander's intent explicitly forbids.

### Alternative D — Require N ≥ 1000 samples

**Description:** Set the minimum sample count at 1000 for a tighter P99 estimate.

**Why considered:** Statistically, more samples improve the tail estimate.

**Why rejected:** At 200 samples, cold-start bench time is already ~100 s at a
P50 of 500 ms. 1000 samples would be ~500 s (over 8 minutes) per bench run.
Combined with multiple latency profiles (`nominal-s3` + `slow-s3`) this becomes
prohibitive in a CI run. 200 samples is the minimum that puts two data points at
the 99th percentile tail (samples 199 and 200 of 200 sorted), which is sufficient
to detect gross SLA violations. A future perf-engineer task may raise this for
formal certification runs while keeping CI at 200.

---

## Consequences

### Positive

- Closes the measurement-validity confound: the rubric-grader can only accept a
  Cat. 3 = 100 claim backed by evidence that satisfies all five rules, making
  "fast only when warm" impossible to slip through measurement.
- Advances **Cat. 3** toward the 100-anchor ("benchmark on the mock meets it …
  no reliance on cache").
- Shared with **Cat. 9** (100-anchor: "a test proving the cold SLA holds with
  caching disabled") — the same bench run satisfying Rule 1.2 + Rule 5 provides
  Cat. 9 evidence at no additional cost.
- Self-describing `benchmark-history.jsonl` entries allow the grader to
  programmatically distinguish cold/warm and on/off-cache results without
  human interpretation.

### Negative / trade-offs

- Each cold-start sample is substantially slower than a warm criterion iteration.
  200 samples at a realistic P50 of 300–700 ms means 60–140 s of bench time per
  profile. CI must allocate a dedicated bench job with sufficient time budget.
- `iter_custom` integration with criterion requires more boilerplate than
  `iter()`. The bench author must understand the constraint and not regress to
  `iter()`.
- Fresh-process spawning (Rule 2, preferred approach) is platform-specific at the
  process-spawning level. Must be tested on Linux (the CI platform) and macOS
  (developer machines).

### Open questions

1. **Cache config key name.** Rule 1.2 references `cache.enabled = false`. The
   exact configuration key depends on the Cat. 9 implementation (EPIC-008), which
   is not yet built. The bench binary should assert the config at startup; the
   exact key name will be filled in when EPIC-008 lands. Until then, the bench
   may stub the assertion.

2. **`iter_custom` sufficiency.** The criterion `iter_custom` API may not
   cleanly suppress warm-up on all versions. The bench author must verify against
   the version pinned in `Cargo.lock`. If it cannot, Alternative B (bespoke
   sampler) applies.

3. **CI bench job timing budget.** 200 samples × 2 profiles = 400 samples.
   At P50 = 700 ms (pessimistic, with process-spawn overhead), that is ~280 s.
   This needs a dedicated CI bench step, not the regular `cargo test` job. The
   CI configuration (`.github/workflows/`) is not yet written; this must be
   addressed when T-0005 (CI wiring) lands.

4. **`drop_caches` on Linux CI.** Dropping the OS page cache between samples
   requires root or `CAP_SYS_ADMIN`. The fresh-process approach (Rule 1.1,
   option 1) avoids this requirement and is preferred. CI should default to
   the fresh-process path; the `drop_caches` path is documented as a fallback
   for environments where process-spawn overhead is prohibitive.

---

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 3 | Latency envelope + SLA | Defines the sole valid evidence path for Cat. 3 = 100; closes the warm-up confound and the unnamed-profile gap. |
| 9 | Caching (resource-aware, optional) | Rule 1.2 + Rule 5 `cache: "off"` field satisfies the 100-anchor's CI-enforced cache-off test. |
| 10 | Tests/coverage/benches | Rule 5 extends the `benchmark-history.jsonl` schema; Rule 2 updates the bench harness design. |

---

## Grader evidence rule (normative, read by `rubric-grader`)

The rubric-grader **must** apply the following filter when computing the Cat. 3
score from `.project/reports/benchmark-history.jsonl`:

```
valid_cold_start_evidence = entries where:
  cold == true
  AND cache == "off"
  AND latency_profile IN ("nominal-s3", "slow-s3")
  AND samples >= 200
```

An entry that fails any filter condition **must not** be used to justify a Cat. 3
score above 50 ("analytical cost model committed"). The 100-anchor additionally
requires that at least one valid entry with `latency_profile == "nominal-s3"` has
`passed == true` (i.e., `p99_ms <= sla_target_ms`).

A `latency_profile == "slow-s3"` entry with `passed == true` provides the
hard-ceiling evidence (P99 ≤ 2 s), which is required for full Cat. 3 = 100 under
the rubric ("benchmark meets it").

---

## Cross-references

- `SPIKE-0007` — the board item this ADR resolves.
- `.project/decisions/0010-perf-sla-ratification-pass.md` — parent finding.
- `SPIKE-0006` — establishes K_min·L_p99 latency floor; determines which profiles
  are analytically plausible for the target SLA.
- `EPIC-003` — parent epic for the latency theorem and Cat. 3 deliverables.
- `docs/process/testing-and-benchmarks.md` §5 (criterion benches) and §7
  (injected-latency profiles) — this ADR amends those sections; they should be
  updated to reference this document.
- `EPIC-008` / Cat. 9 — the `cache.enabled` config key this protocol depends on.
- `T-0005` — CI wiring; must allocate the cold-start bench job.

---

## Sign-off

### Adversarial review record

_(no rounds yet)_

### Steering ratification

_(pending adversarial review — owner: steering-perf-sla)_
