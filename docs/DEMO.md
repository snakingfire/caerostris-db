# caerostris-db — end-to-end demo

A minimal, fully working round trip: **insert graph data → run an openCypher
`MATCH` query → see the inserted data returned.** This is the hackathon-video
deliverable; it wires the already-landed data model (`src/model/`) and Cypher
parser (`src/cypher/`) into a tiny store and `MATCH ... RETURN`
executor (`src/demo/`).

There are two demos:

| Demo | Command | What it proves |
|------|---------|----------------|
| **Object-storage-native** (the headline) | `./scripts/demo-minio.sh` | The graph's durable state is **real S3 objects** in MinIO; queries read them back. |
| In-memory | `./scripts/demo.sh` | The bare insert → `MATCH` → return round trip, no storage backend. |

---

## ⭐ Object-storage-native demo (`scripts/demo-minio.sh`)

**The wow:** caerostris-db is a graph database whose source of truth is plain
object storage. This demo persists a social graph as individual S3 objects in
the local MinIO bucket, lists those objects straight from S3, then answers
openCypher `MATCH` queries by reading them back.

```bash
./scripts/demo-minio.sh
```

It (0) provisions the local MinIO mock and an isolated bucket/prefix, then:

1. **Shows the bucket EMPTY** (`aws s3api list-objects-v2 …` — no objects yet).
2. **Inserts** a 6-node / 7-edge social graph (4 `:Person`, 2 `:Company`, with
   `KNOWS` and `WORKS_AT` relationships) and **persists** it — each node and
   edge becomes its own object: `nodes/<id>.json`, `edges/<id>.json`.
3. **Shows the bucket now CONTAINS** those objects (real S3 keys + byte sizes).
4. **Reads the graph back out of S3** and runs four openCypher queries,
   including a multi-property filter, a one-hop `WORKS_AT` traversal, and a
   one-hop `KNOWS` traversal with a `WHERE` clause.

### The S3 backend

`src/storage/s3_cli.rs` implements the engine's `ObjectStore` trait
(`put`/`get`/`get_range`/`delete`/`list`) against any S3-compatible endpoint by
shelling out to the `aws s3api` CLI — **zero new Rust dependencies**, works
offline against the swarm's MinIO. `src/demo/persist.rs` serialises the graph
to/from objects through that trait, so the *same* persist/query code path runs
over `MemoryStore` in unit tests and over real S3 in the integration tests
(`tests/s3_minio_integration.rs`) and the demo.

### Equivalent direct invocations

```bash
# Provision env + an isolated bucket first:
source .project/env/local.env
eval "$(scripts/env/bucket.sh demo)"

cargo run --quiet --bin caero -- minio-demo   # the full narrated S3 demo
cargo run --quiet --bin caero -- s3-ls        # list the bucket via the engine's own backend
```

### Expected output (abridged)

```
[1/4] The S3 bucket starts EMPTY
  (no objects — the durable graph does not exist yet)

-- 2. Insert a social graph & persist it as objects --
  built 6 nodes and 7 edges
  wrote 13 objects to the store

-- 3. The object store now holds the durable graph --
  nodes/0.json         267 bytes
  edges/0.json         155 bytes
  ...

-- Q3: one-hop traversal (who works where) --
  query : MATCH (p:Person)-[r:WORKS_AT]->(c:Company) RETURN p, c
  result:
  row 1: p = (:Person {age: 30, city: 'Berlin', name: 'Alice'}), c = (:Company {city: 'Berlin', name: 'Acme'})
  ...

-- Q4: one-hop + WHERE clause (Alice's acquaintances) --
  query : MATCH (a:Person)-[:KNOWS]->(friend) WHERE a.name = 'Alice' RETURN friend
  result:
  row 1: friend = (:Person {age: 27, city: 'Berlin', name: 'Bob'})
  row 2: friend = (:Person {age: 41, city: 'Lisbon', name: 'Carol'})
```

The same keys also appear in `aws s3api list-objects-v2` output — proving the
durable graph state is genuine object storage, not an in-process illusion.

---

## In-memory demo (`scripts/demo.sh`)

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
