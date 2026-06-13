# Coverage Report — T+3:15 landing of T-0005

Generated during integrator landing of T-0005 onto main (commit `0dd2f2d`).

## Summary

| Metric | Value |
|--------|-------|
| Line coverage | **96.29%** |
| Lines covered | 2131 / 2213 |
| Tests run | 176 passed, 0 skipped |
| Coverage gate threshold | 0% (ratcheting toward 90%) |
| Gate status | PASS |

## Per-file breakdown

| File | Lines | Missed | Cover |
|------|-------|--------|-------|
| src/lib.rs | 6 | 0 | 100.00% |
| src/licenses.rs | 254 | 2 | 99.21% |
| src/main.rs | 4 | 4 | 0.00% |
| src/model/edge.rs | 63 | 0 | 100.00% |
| src/model/node.rs | 73 | 0 | 100.00% |
| src/model/schema.rs | 109 | 0 | 100.00% |
| src/model/value.rs | 369 | 25 | 93.22% |
| src/query/stats.rs | 267 | 9 | 96.63% |
| src/storage/memory.rs | 92 | 1 | 98.91% |
| src/storage/mod.rs | 76 | 11 | 85.53% |
| src/tck.rs | 73 | 0 | 100.00% |
| tck-runner/src/engine.rs | 31 | 1 | 96.77% |
| tck-runner/src/lib.rs | 44 | 4 | 90.91% |
| tck-runner/src/main.rs | 119 | 7 | 94.12% |
| tck-runner/src/report.rs | 98 | 0 | 100.00% |
| tck-runner/src/runner.rs | 156 | 4 | 97.44% |
| tck-runner/src/scenario.rs | 379 | 14 | 96.31% |
| **TOTAL** | **2213** | **82** | **96.29%** |

## Tool versions

- cargo-llvm-cov: 0.8.7
- LLVM: 21.1.8
- Rust: 1.96.0

## Command used

```
LLVM_COV=/nix/store/zbblxnd78j10s7gv8d2g8msvjiamrl88-llvm-21.1.8/bin/llvm-cov \
LLVM_PROFDATA=/nix/store/zbblxnd78j10s7gv8d2g8msvjiamrl88-llvm-21.1.8/bin/llvm-profdata \
cargo llvm-cov nextest --all-features --workspace --summary-only
```

## Notes

- `src/main.rs` shows 0% coverage — this is the binary entry point, not exercised by the unit/integration test suite (expected).
- `src/storage/mod.rs` is at 85.53% — below 90% for this module, but workspace total well above threshold. BUG can be filed as debt if needed.
- Coverage gate is at `COVERAGE_THRESHOLD: 0` per Cat. 10 ratchet policy; threshold will be raised as engine tests land.
