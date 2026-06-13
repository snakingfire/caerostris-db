---
id: EPIC-009
title: Test harness, ≥90% coverage, datasets, benchmarks
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [10]
estimate: L
created: T0
updated: T0
---

## Context

Test infrastructure (Cat. 10, weight 8, GATE) is a first-class deliverable, not an afterthought. The rubric requires ≥90% line coverage reported by **cargo-llvm-cov** in CI, integration tests against a **local S3 mock** (MinIO or moto), and **criterion benchmarks** for the headline query and aggregates, tracked over time.

This epic covers: (1) the full test infrastructure wiring — unit tests, integration tests, property-based tests, and the TCK runner (T-0002); (2) cargo-llvm-cov configured to report coverage in CI and fail the build below 90%; (3) a local MinIO/moto-based integration test harness that spins up and tears down per test run (T-0001 provides the object-store abstraction, this epic provides the test fixtures); (4) criterion benchmark suite for the headline 6-hop query, aggregate queries, and commit throughput, with results committed as baseline artifacts; and (5) representative synthetic datasets for benchmarking (license-clean, generated or from open sources).

Coverage must not regress: every PR must leave coverage ≥ 90% or provide justification for a temporary exception. The grader reads the coverage% from CI output.

Relevant requirements: R12 (≥90% coverage, integration tests on mock, criterion benches, hourly releases), R10 (TCK pass-rate in CI).

## Acceptance criteria

- [ ] cargo-llvm-cov configured and running in CI; a coverage report is generated per commit; the build fails if line coverage drops below 90%.
- [ ] Integration test harness: MinIO (or moto for Python-side tests) spun up in CI for each run; integration tests exercise the full read/write/commit path against the mock.
- [ ] Criterion benchmark suite: benchmarks for (a) the headline 6-hop in-envelope query, (b) aggregates (count, sum, distinct), (c) commit throughput; baselines committed to `benches/baselines/`.
- [ ] Benchmark regression detection: CI warns (or fails) if a benchmark regresses by more than a configured threshold vs. the committed baseline.
- [ ] Synthetic datasets committed (or generation scripts committed) for benchmarking: at least one 1M-node / 10M-edge graph with text properties, license-clean.
- [ ] TCK harness (from T-0002) reports pass-rate as a CI output in a format the rubric grader can parse.
- [ ] Property-based tests (proptest or similar) cover ACID invariants: arbitrary write sequences produce consistent reads.
- [ ] `./format_code.sh` green; CI green.

## Notes / log

T-0005 is the kickoff task for CI coverage and grader inputs. T-0002 (TCK harness) is a co-dependency. The 90% coverage target must be tracked from day one, not retrofitted at the end.
