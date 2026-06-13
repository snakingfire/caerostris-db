# caerostris-db — agent guide

A graph database engine built from the ground up in Rust on commodity durable
object storage (S3-like), inspired by DuckDB: ACID + transactional, embedded or
server, single-writer/multi-reader, custom object-storage-native format, full
openCypher, with a **formally provable cold-start latency target**. Named for
*Caerostris darwini*, the spider with the toughest silk known.

**Status:** requirements are defined and the project is built by an **autonomous
agent swarm**. Read these first, in order:

1. [`docs/commanders-intent.md`](docs/commanders-intent.md) — the north star.
2. [`docs/requirements/master-rubric.md`](docs/requirements/master-rubric.md) —
   the single graded source of truth (weighted, gated).
3. [`docs/requirements/core-requirements.md`](docs/requirements/core-requirements.md) —
   R1–R12, incl. the **latency selectivity-envelope theorem** (read it twice).
4. [`docs/process/autonomous-operating-model.md`](docs/process/autonomous-operating-model.md) —
   roles, cadence, agile-parallel doctrine, pace.

**How work happens:** pull a ready item from the board
([`.project/board/`](.project/board/), protocol in
[`docs/process/task-board-protocol.md`](docs/process/task-board-protocol.md)),
build it TDD-first in an isolated worktree, open a simulated PR
([`docs/process/simulated-pr-workflow.md`](docs/process/simulated-pr-workflow.md)),
clear the **adversarial review + pre-mortem** gate
([`docs/process/adversarial-review-loops.md`](docs/process/adversarial-review-loops.md)),
and let the integrator land it. **Designs** additionally pass steering-committee
sign-off ([`docs/process/steering-committee.md`](docs/process/steering-committee.md))
and **design-before-code** ordering
([`docs/process/formal-verification-policy.md`](docs/process/formal-verification-policy.md)).
The run is launched with `/launch`; supervision lives in
[`RUNBOOK.md`](RUNBOOK.md). The orchestrator is `.claude/workflows/mainspring.js`;
agent definitions are in `.claude/agents/`.

**Non-negotiable invariant:** the cold-start P99 ≤ 1 s target is a *conditional*
theorem over a selectivity/byte-budget envelope, and must hold **without** the
cache. Anything implying "fast only when warm" or "fast only with luck" is a
design falsification — escalate to steering.

## Dev workflow

This is a Rust project. Tooling comes from a Nix `devenv` shell (`flake.nix`),
auto-loaded via direnv (`direnv allow`); non-Nix users get the same stable
toolchain via `rust-toolchain.toml` + rustup. Integration tests run against a
local S3 mock (MinIO/moto) — see
[`docs/process/testing-and-benchmarks.md`](docs/process/testing-and-benchmarks.md).

```bash
cargo build
cargo test                                   # unit tests + doctests + integration
cargo nextest run                            # faster, in the Nix shell
cargo llvm-cov --summary-only                # coverage (target ≥90%)
cargo clippy --all-targets -- -D warnings
cargo bench                                  # criterion benches
./format_code.sh                             # ALWAYS run before committing
```

## Conventions

- **Run `./format_code.sh` before every commit/landing** (cargo fmt + clippy -D
  warnings + taplo). CI enforces fmt, clippy, and tests.
- **Clippy warnings are errors.** Keep the tree warning-clean.
- **≥90% line coverage**, integration tests on the S3 mock, criterion benches.
- **Open source, public repo: never commit secrets or data.** gitleaks runs in
  pre-commit; `.env*`, keys, `/target`, and large datasets/artifacts are
  gitignored. License-clean deps + datasets only. See
  [`docs/process/open-source-guardrails.md`](docs/process/open-source-guardrails.md).
- **`Cargo.lock` is committed** (this crate ships a binary).
- **Never use destructive git** (`reset --hard`, `push --force`, branch deletion)
  without explicit authorization for that exact action.
- Promote `lib.rs`/`main.rs` to a Cargo workspace when the engine splits into
  multiple crates (it will).
- **Keep docs current as you go** — when a change makes a doc false, fix the doc
  in the same change; record decisions (ADRs, `.project/decisions/`). See
  [`docs/process/memory-and-docs-policy.md`](docs/process/memory-and-docs-policy.md).
- **Watch the wallclock** ([`.project/pace/deadline.md`](.project/pace/deadline.md))
  and **never block the board.** When uncertain, decide toward the commander's
  intent, record why, and keep moving.
