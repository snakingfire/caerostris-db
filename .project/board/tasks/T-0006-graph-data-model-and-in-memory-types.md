---
id: T-0006
title: Define core graph data-model types (Node, Edge, PropertyValue, Schema)
type: task
status: done
priority: P1
assignee: integrator
epic: EPIC-001
deps: [T-0001]
rubric_refs: [1, 2, 4]
estimate: S
created: T0+0:20
updated: T+2:43
---

## Context

Every layer (storage format, commit, planner, TCK adapter, Python bindings) needs
a shared, in-memory representation of the property graph: nodes with labels and
properties, directed typed edges with properties, and the openCypher value type
system. This is deliberately **independent of the on-object format** (SPIKE-0003)
— it is the logical model that format writers/readers serialise to/from, so it can
land before the format spec is ratified. Keep it small and stable; the byte layout
comes later. See `EPIC-001` and `docs/requirements/core-requirements.md` (R1).

## Acceptance criteria
- [ ] `PropertyValue` enum covers all openCypher scalar + container types: null, boolean, integer (i64), float (f64), string, list, map.
- [ ] `Node` (id, labels, properties), `Edge` (id, type/label, source id, target id, properties), and a `Schema`/catalog stub (known labels, rel-types, property keys) defined with `///` docs.
- [ ] Value equality + ordering follow openCypher semantics (null handling, mixed-type comparison) — unit-tested against the cases the TCK will exercise.
- [ ] Types are `Clone` + serde-serialisable so downstream layers can round-trip them in tests without depending on the on-object format.
- [ ] tests added (unit); coverage not regressed
- [ ] docs / ADR / CLAUDE.md updated if behaviour or architecture changed
- [ ] `./format_code.sh` green

## Notes / log
Independent of SPIKE-0003 by design — the format slots underneath these logical
types. Do not bake byte-layout assumptions into these types.

T+2:43 — Landed in commit 6a80b76 by integrator (serial land under pace-marshal authority).
Implemented: src/model/{mod,value,node,edge,schema}.rs; 60 new unit tests; all 99 tests green;
./format_code.sh green. serde + serde_json added to Cargo.toml from workspace.
