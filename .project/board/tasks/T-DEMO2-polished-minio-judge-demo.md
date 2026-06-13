---
id: T-DEMO2
title: Polished MinIO-backed wow demo for the 1-minute judge video
type: task
status: in_progress
priority: P0
assignee: focused-builder
epic: EPIC-009
deps: []
rubric_refs: [2, 4, 12]
estimate: M
created: 2026-06-13T22:25:00Z
updated: 2026-06-13T22:25:00Z
---
## Context
Human request (T+4:01): a POLISHED, IMPRESSIVE end-to-end demo for a 1-minute hackathon
judge video that grades the whole project. Must have WOW that proves
"object-storage-native graph DB."

## Acceptance criteria
- [ ] Show the MinIO/S3 bucket EMPTY.
- [ ] Insert graph data (people + relationships) via `caero`.
- [ ] Show the bucket NOW CONTAINS the persisted data objects (durable state = real S3 objects).
- [ ] openCypher MATCH queries READ FROM the bucket and return the inserted data.
- [ ] A few MORE COMPLEX queries (multi-property filter, one/two-hop, WHERE).
- [ ] Polished, labeled, screen-recordable output; `scripts/demo-minio.sh` + docs/DEMO.md.
- [ ] Requires a minimal S3 ObjectStore backend (object_store crate vs MinIO) — the missing keystone.
- [ ] format_code.sh + tests green; integration test on MinIO.
- FALLBACK if S3 backend not feasible in time: polish the in-memory demo (richer/complex queries, nicer output).

## Notes / log
Dispatched to a focused builder at T+4:01 (agent a8120ba). The wow keystone is the S3/MinIO
ObjectStore backend (main only has MemoryStore). Storage writers T-0007/0008 building in parallel.
