# caerostris-db — end-to-end demo

A minimal, fully working round trip: **insert graph data → run an openCypher
`MATCH` query → see the inserted data returned.** This is the hackathon-video
deliverable; it wires the already-landed data model (`src/model/`) and Cypher
parser (`src/cypher/`) into a tiny in-memory store and `MATCH ... RETURN`
executor (`src/demo/`).

## Run it (one command)

```bash
./scripts/demo.sh
```

That builds the `caero` binary and runs `caero demo`, printing labelled
sections suitable for screen recording.

### Equivalent direct invocations

```bash
cargo run --quiet --bin caero -- demo      # the caero CLI
cargo run --quiet -- demo                  # the caerostris-db CLI (same logic)
cargo run --quiet --example demo           # the example binary (same logic)
```

## What it does

1. **Insert** three nodes and one edge into an in-memory graph:
   - `(:Person {name: 'Alice', age: 30})`
   - `(:Person {name: 'Bob'})`
   - `(Alice)-[:KNOWS]->(Bob)`
2. **Query a single node** with a label + property-equality filter:
   `MATCH (p:Person {name: 'Alice'}) RETURN p` — returns Alice.
3. **Query a one-hop relationship**:
   `MATCH (a:Person)-[:KNOWS]->(b) RETURN a, b` — returns the Alice/Bob pair.

## Expected output

```
== caerostris-db end-to-end demo ==

-- 1. Insert data --
  inserted (:Person {name: 'Alice', age: 30}) -> id 0
  inserted (:Person {name: 'Bob'}) -> id 1
  inserted (Alice)-[:KNOWS]->(Bob) -> edge id 0

-- 2. Query a single node --
  query: MATCH (p:Person {name: 'Alice'}) RETURN p
  result:
  row 1: p = (:Person {age: 30, name: 'Alice'})

-- 3. Query a one-hop relationship --
  query: MATCH (a:Person)-[:KNOWS]->(b) RETURN a, b
  result:
  row 1: a = (:Person {age: 30, name: 'Alice'}), b = (:Person {name: 'Bob'})

== demo complete: inserted data returned by MATCH ==
```

## Scope and supported query surface

The demo executor (`src/demo/executor.rs`) is intentionally minimal — the
production planner/executor lands in EPIC-002. It supports:

- **Single node:** `MATCH (n:Label {key: value}) RETURN n` — label filter and
  inline property-equality filter are both optional.
- **One hop:** `MATCH (a:Label)-[:REL]->(b) RETURN a, b` — filters on both
  endpoints; the relationship type is an optional filter.
- **`WHERE`:** a single `var.key = <literal>` equality, e.g.
  `MATCH (p:Person) WHERE p.name = 'Bob' RETURN p`.
- `RETURN` accepts bare variables with optional `AS` aliases.

Anything outside this surface (multi-hop, `RETURN *`, aggregations, write
clauses) returns a structured error rather than a wrong answer.

Data is held in memory (`Vec<Node>` + `Vec<Edge>`); the durable,
object-storage-native store and the full planner replace this in the engine
proper. Inserts go through the store API (`GraphStore::insert_node` /
`insert_edge`), not Cypher `CREATE`, which the parser does not yet cover.
