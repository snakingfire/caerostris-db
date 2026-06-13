---
id: T-0017
title: Implement openCypher lexer + parser to AST
type: task
status: done
priority: P1
assignee: integrator
epic: EPIC-002
deps: [T-0001]
rubric_refs: [4]
estimate: M
created: T0+0:20
updated: T0+3:20
---

## Context

The front of the openCypher pipeline: tokenise and parse Cypher source into a typed
AST. This is independent of storage and can start immediately — it unblocks the
planner and executor and lets the TCK harness (T-0002) start feeding real queries.
Grammar should track the openCypher EBNF for the TCK tag pinned in BUG-0007 /
decision 0008. See `EPIC-002`, `docs/requirements/core-requirements.md` (R10).

## Acceptance criteria
- [ ] Lexer tokenises Cypher (keywords, identifiers, literals of every type, operators, parameters `$x`) with correct error positions.
- [ ] Parser produces a typed AST for read clauses first: MATCH, WHERE, RETURN, WITH, UNWIND, ORDER BY, SKIP, LIMIT, and pattern syntax (nodes, directed/typed rels, var-length stubs).
- [ ] Parse errors are structured (location + message), not panics.
- [ ] A corpus of valid TCK query strings parses without error; a set of invalid strings is rejected with errors.
- [ ] tests added (unit + parser corpus); coverage not regressed
- [ ] docs / ADR updated if grammar scope decisions are made
- [ ] `./format_code.sh` green

## Notes / log
Independent of storage format — ready from now. Write-clause AST (CREATE/MERGE/SET/
DELETE/REMOVE) can extend this in a follow-up (T-0021).

Landed in commit 91b934c at T+3:20. Branch work/T-0017-cypher-parser. Format green,
176/176 tests pass (including 25 lexer + 19 parser unit tests, cypher corpus, and full
TCK pass-rate suite). Pace-marshal direct land per dispatch T+3:15.
