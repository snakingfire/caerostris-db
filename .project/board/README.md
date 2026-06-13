# Board — `.project/board/`

File-based work tracker. One file per item in `tasks/`. Full protocol:
[`docs/process/task-board-protocol.md`](../../docs/process/task-board-protocol.md).

## TL;DR for agents

1. **Find work:** `scripts/board/ls.sh ready` (or `rg 'status: ready' tasks/`).
   Prefer highest `priority` then highest-weight `rubric_refs`.
2. **Claim:** edit the file → set `assignee`, `status: in_progress`, `updated`. Commit `board: claim <ID>`.
3. **Work:** TDD-first, in a worktree (see `docs/process/simulated-pr-workflow.md`).
4. **Review:** open a simulated PR → adversarial reviewer + pre-mortem must sign off.
5. **Land:** integrator merges to `main` with `format_code.sh` + tests green. Set `status: done`, note the commit.
6. **File bugs freely** as `BUG-NNNN`. **Split big tasks.** **Never block the board.**

## States

`backlog → ready → in_progress → in_review → done`  (also `blocked`, `dropped`).

## Seeded epics

| ID | Epic | Rubric |
|----|------|--------|
| EPIC-001 | Object-storage-native storage format & ACID commit protocol | 1,2 |
| EPIC-002 | openCypher engine → 100% TCK | 4,6 |
| EPIC-003 | Latency selectivity-envelope theorem + cold-start SLA | 3,11 |
| EPIC-004 | ACID transactions + TLA+ formal verification | 1,11 |
| EPIC-005 | Pluggable secondary indices (B-tree on text first) | 5 |
| EPIC-006 | Concurrency & the four attach modes (embedded ×3 + server) | 7 |
| EPIC-007 | Python embedded bindings | 8 |
| EPIC-008 | Resource-aware optional caching | 9 |
| EPIC-009 | Test harness, ≥90% coverage, datasets, benchmarks | 10 |
| EPIC-010 | Harden the autonomous harness (self-expansion) — **P0** | 12 |

New numbers: `ls tasks/`, take max+1. Collisions are harmless — rename.
