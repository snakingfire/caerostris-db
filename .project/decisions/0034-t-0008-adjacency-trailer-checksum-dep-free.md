# Decision 0034 — T-0008 dependency-free `.adj` trailer checksum + property test

- **Status:** accepted (local, reversible implementation decisions — logged, not
  steering-gated).
- **Date / T+:** T0+~3:50.
- **Owner:** implementer-wf_fe688db0-093-33 (T-0008).
- **Scope:** two dependency-avoidance choices in T-0008: (1) the `.adj` **trailer
  self-checksum** (ADR 0008 §3.2 `blake3_prefix`); (2) the **round-trip property
  test** generator (board AC #3 says "proptest"). Neither touches the
  content-addressing object-key hash (ADR 0002 §1 / SPIKE-0004 §4.4 — that BLAKE3
  digest names the `db/data/<content-hash>/...` key and is owned by the
  writer/manifest layer, T-0009).

## Decision 2 — property test uses the in-repo `SplitMix64`, not the `proptest` crate

Board AC #3 asks for a property test over "arbitrary directed typed edge sets".
Adding the `proptest` crate would pull a large transitive tree (`rand`,
`bitflags`, `rusty-fork`, `quick-error`, `unarray`, `lazy_static`, …), each of
which must be recorded in `docs/licenses/manifest.toml` to keep
`tests/license_manifest.rs` + `cargo-deny` green — a sizeable Cat. 12 change, and
a `Cargo.lock` churn that risks rebase conflicts against the other storage-cascade
PRs landing concurrently (T-0007/T-0009/T-0010).

The crate already vendors a deterministic, license-clean `SplitMix64` RNG
(`src/dataset/rng.rs`, decision behind T-0035) *precisely* to avoid the `rand`
dependency. T-0008's property test reuses it: a seeded generator produces
hundreds of randomized edge sets with randomized targets, edge ids, and
randomized property maps (all `PropertyValue` kinds incl. nested lists/maps,
NaN, empty strings, unicode) and asserts byte-exact round-trip fidelity through
the writer/reader. This delivers the *falsification intent* of property testing
(arbitrary inputs round-trip identically) with zero new license surface, and is
**deterministic** (seeded) so a failure is reproducible — arguably better for a
CI gate than a randomly-seeded proptest run. If the project later adopts
`proptest` wholesale (e.g. a `tests/` shrinking harness), this test can be ported;
the generator is small and the invariant unchanged.

## Decision 1 — `.adj` trailer self-checksum is dependency-free (not BLAKE3 yet)

## Context

ADR 0008 §2.2/§3.2 specify a 16-byte trailer ending each `.ncol`/`.adj` object:
`{ dir_off: u64, blake3_prefix: [u8;8] }`. The trailer exists for **fail-closed
integrity + tool/recovery** discovery of the directory offset (§8.2/§8.3) — it is
*not* on the latency hot path (the manifest partition map carries the directory
offset inline; §2.2). The hot-path content-address key hash is a separate concern.

Pulling `blake3` into the **engine crate's** dependency tree right now would add
the crate plus its transitive deps (arrayref, arrayvec, constant_time_eq,
cfg-if, …), each of which must be recorded in `docs/licenses/manifest.toml` and
pass `tests/license_manifest.rs` + `cargo-deny`. That is meaningful Cat. 12 churn
for a checksum whose only current consumer is T-0008's fail-closed framing guard.

## Decision

Implement the trailer self-checksum as a **dependency-free 64-bit FNV-1a digest**
over the object bytes preceding the trailer, truncated to the 8-byte trailer
field. The reader recomputes it and **fails closed** (`StorageFormatError`) on
mismatch, exactly satisfying the fail-closed integrity intent of ADR 0008
§8.2/§8.3. The header `magic`/`format_version`/`object_kind` are likewise
validated fail-closed.

## Why this does not falsify ADR 0008

- The trailer checksum is **not** load-bearing for correctness of the latency
  proof, the byte budget, `r ≤ 1`, or early-abort — it is an integrity guard for
  tools/recovery (§8.3). Its specific hash algorithm is an implementation detail.
- The **content-addressed object key** (the BLAKE3-named `db/data/<hash>/...` key
  that the GC reference-set test and cross-version de-dup depend on, ADR 0002 §1 /
  §7.2, decision 0027 BC-1) is **unchanged and out of T-0008's scope** — it is the
  writer/manifest layer's job (T-0009). T-0008 emits the `.adj` bytes; whoever
  writes the object key computes the BLAKE3 of those bytes. No GC/de-dup invariant
  is affected by the *trailer* checksum algorithm.
- `format_version` in the header gates forward-compat: when the engine adopts
  BLAKE3 (e.g. when T-0009 needs it for keying), the trailer can switch to a
  BLAKE3 prefix under a bumped `format_version` with a fail-closed older reader —
  no silent reinterpretation (§8.2). This decision is therefore **reversible**.

## Consequences

- No new engine dependency; Cat. 12 license manifest untouched by T-0008.
- ADR 0008 cross-reference annotated (T-0008 records this trailer deviation per
  the task's AC "docs / ADR updated if format detail deviates from SPIKE-0003").
- Follow-up (non-blocking): when T-0009 introduces BLAKE3 for content-address
  keying, unify the trailer checksum onto a BLAKE3 prefix behind a
  `format_version` bump. Tracked here; not a blocker for T-0008.

## Alternatives considered

- **Add `blake3` now.** Rejected for this task: license-manifest + lockfile churn
  for a non-hot-path integrity guard, and the keying hash (the real BLAKE3
  consumer) is T-0009's. Premature here; deferred to whoever first needs the
  content-address key.
- **Omit the trailer checksum.** Rejected: ADR 0008 §8.2/§8.3 require fail-closed
  self-description and integrity; an omitted checksum would be a real format
  deviation, not a cosmetic one.
</content>
