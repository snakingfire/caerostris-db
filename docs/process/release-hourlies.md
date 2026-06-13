# Hourly Releases — caerostris-db

> **Rubric anchor:** Cat. 12 (Engineering & process health). The rubric requires
> ≥1 hourly release per hour of the run. Each missed hourly is a Cat. 12 gap.
> See [`../requirements/master-rubric.md`](../requirements/master-rubric.md).

---

## Purpose

While the swarm implements in parallel, human reviewers and automated graders need
a **stable, named artifact** to test against. The hourly release provides that:
a known-good snapshot of the tree cut approximately every 60 minutes, with auto-
generated release notes that include the current rubric score and the board items
landed since the last hourly.

Hourlies are **for parallel testing** — a tester or grader can pull a specific
hourly and run the test + benchmark suite without worrying that half-built work has
been pushed since.

---

## Cadence

- **Every ~60 minutes** on the wallclock, the `pace-marshal` agent triggers the
  hourly release process.
- The release must complete within ~5 minutes of the trigger. A release that takes
  longer indicates a flaky test suite — fix it.
- A **failing hourly is a P0 stop-the-line event.** If `scripts/release-hourly.sh`
  exits non-zero, the pace-marshal files a P0 task, pages the swarm, and does not
  tag until the tree is green.

---

## Artifact types

| Artifact | Location | Committed to git? |
|---|---|---|
| Git tag `hourly-<N>` or `hourly-<YYYYMMDD-HHMM>` | git tags | yes |
| Release notes | `releases/hourly-<N>.md` | **yes** |
| Built release binary (`caerostris-db`) | build artifact | **no** (gitignored) |
| Python wheel (`caerostris_db-*.whl`) | build artifact | **no** (gitignored) |
| Coverage report (`lcov.info`) | build artifact | **no** (gitignored) |
| TCK results JSON | `.project/reports/tck-latest.json` | yes |
| Benchmark history entry | `.project/reports/benchmark-history.jsonl` | yes |

Binary artifacts and wheels are produced during the release but are **not
committed** to git (they live in `target/`, which is gitignored). The release
notes and the tag are the durable, committed record. If a downstream consumer
needs the binary, they `git checkout hourly-<N> && cargo build --release`.

---

## What `scripts/release-hourly.sh` does

The script is the single entry point for cutting a release. It:

1. **Verifies a green tree:**
   ```bash
   ./format_code.sh                              # fmt + clippy -D warnings + taplo
   cargo nextest run                             # all tests pass
   ```
   If either step fails, the script exits non-zero immediately (no tag, no notes).

2. **Runs coverage and records it:**
   ```bash
   cargo llvm-cov nextest --lcov --output-path lcov.info
   # Extracts the line-coverage percentage and writes it to a temp variable.
   ```

3. **Runs the criterion headline benchmark** (tiny fixture, no mock needed) and
   appends a result entry to `.project/reports/benchmark-history.jsonl`.

4. **Reads the latest rubric grade** from `.project/reports/` (the most recent
   file written by the rubric-grader).

5. **Reads landed board items since the last hourly** from `.project/board/` (all
   items with `status: done` and a `closed_at` timestamp after the previous
   hourly's tag).

6. **Determines the next release number** by counting existing `hourly-*` tags.

7. **Generates release notes** at `releases/hourly-<N>.md`:
   - Header: release number, timestamp, git SHA.
   - Current rubric score (overall + per-category scores from the latest grade).
   - Coverage percentage.
   - List of landed board items since the last hourly (title + rubric_refs).
   - Open P0/P1 gaps (items with `priority: P0` or `P1` and `status: blocked` or
     `todo`).

8. **Commits the release notes file and any updated report files:**
   ```bash
   git add releases/hourly-<N>.md \
           .project/reports/benchmark-history.jsonl \
           .project/reports/tck-latest.json
   git commit -m "chore: hourly release $(N) — rubric $(SCORE)%"
   ```

9. **Tags the commit:**
   ```bash
   git tag hourly-<N>
   ```

10. **Builds the release binary and Python wheel** (for local use by testers):
    ```bash
    cargo build --release
    # (maturin build --release  when Python bindings are ready)
    ```
    These are not committed; they land in `target/release/`.

### Running manually

```bash
bash scripts/release-hourly.sh
```

The script is idempotent: running it twice in a row without new commits produces a
second tag and notes file rather than clobbering the first.

---

## Release notes format

```markdown
# Hourly Release N — caerostris-db

**Timestamp:** 2026-06-13T14:00:00Z
**Git SHA:** abc1234
**Rubric score:** 62 / 100
**Line coverage:** 71%

## Rubric scores this release

| Cat | Name | Score | Gate? |
|-----|------|------:|:-----:|
|  1  | ACID txns | 50 | ✓ |
|  2  | Storage format | 50 | ✓ |
| ... | ... | ... | ... |

## Landed since last hourly

- T-0042: Implement manifest atomic swap (Cat. 2) — `.project/board/tasks/T-0042-manifest-atomic-swap.md`
- T-0038: Property-based tests for commit protocol (Cat. 1, 10) — ...

## Open P0/P1 gaps

- T-0055 [P0]: TCK harness not yet wired (Cat. 4)
- T-0061 [P1]: Coverage at 71%, below 90% gate (Cat. 10)
```

---

## Relationship to the grader and pace-marshal

- The **rubric-grader** runs every 20 minutes independently and writes to
  `.project/reports/`. The hourly release reads the grader's latest output — it
  does not re-score.
- The **pace-marshal** triggers the hourly release at the ~60-minute mark of each
  epoch. If the pace-marshal determines the tree is not releasable (failing
  tests, red CI), it files a P0 and attempts a release at the next 10-minute
  cycle until it succeeds.
- Hourlies do not block swarm work — agents continue landing changes while the
  release script runs. The tag is cut on the commit that was HEAD at trigger time.
