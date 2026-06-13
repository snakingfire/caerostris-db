---
id: T-0032
title: Python packaging polish + per-attach-mode pytest coverage
type: task
status: backlog
priority: P3
assignee:
epic: EPIC-007
deps: [T-0031, T-0029]
rubric_refs: [8]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 8 = 100 requires open/attach in all four modes from Python and a pytest suite
covering them. This task completes server-mode Python attach (over T-0029),
finalises wheel packaging metadata, and ensures one pytest per attach mode. See
`EPIC-007`.

## Acceptance criteria
- [ ] Server-mode attach works from Python (connects to a T-0029 server, runs a query).
- [ ] pytest covers all four attach modes (writer-master, read-only, master-less, via-server).
- [ ] Wheel metadata complete (name, version, classifiers, license = permissive); `maturin build` wheel installs and imports cleanly in a fresh venv (tested in CI).
- [ ] tests added (pytest per mode); coverage not regressed; `ruff`/`flake8` clean
- [ ] docs updated with install + usage
- [ ] `./format_code.sh` green for the Rust side

## Notes / log
Design-before-code: depends on T-0031 + T-0029 (server mode). P3 polish — pull
after the core Python API and server mode land.
