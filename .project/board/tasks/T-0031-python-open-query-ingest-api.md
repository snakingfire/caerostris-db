---
id: T-0031
title: Python open/attach + parameterized query + ingest API (native results)
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-007
deps: [T-0030, T-0019, T-0027]
rubric_refs: [8]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The real Python API over the engine: open/attach in each mode, run parameterized
openCypher queries, ingest nodes/edges, and get results back as native Python
objects (lists of dicts; values as Python int/float/str/bool/list/dict — not opaque
handles). Wraps the Rust public API; does not re-implement logic. Depends on the
read query path (T-0019) and the attach modes (T-0027). See `EPIC-007`.

## Acceptance criteria
- [ ] `open`/`attach` exposed for the embedded modes (writer-master, read-only, master-less); server-mode attach can follow once T-0029 lands.
- [ ] Parameterized queries: `db.query("MATCH (n) WHERE n.id = $id RETURN n", id=42)` returns results.
- [ ] Ingest: insert nodes/edges (or bulk-load from a dict/list) and query them back.
- [ ] Results as native Python objects: lists of dicts with Python-native value types, not opaque Rust handles.
- [ ] Domain errors surface as typed Python exceptions.
- [ ] pytest suite in CI: ≥1 integration test per embedded mode + one ingest→query round-trip, all against the local S3 mock (moto).
- [ ] tests added (pytest integration); coverage not regressed; `ruff`/`flake8` clean
- [ ] `./format_code.sh` green for the Rust side

## Notes / log
Design-before-code: depends on T-0030 (scaffold), T-0019 (read path, gated on
SPIKE-0003), T-0027 (attach modes). Server-mode Python attach piggybacks on T-0029.
