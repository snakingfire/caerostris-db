# Vendored openCypher TCK

This directory contains the **openCypher Technology Compatibility Kit (TCK)**
Gherkin `.feature` files, vendored into the repo so the TCK harness
(`tck-runner/`) can run in CI **without external network access** (an acceptance
requirement of board item `T-0002`).

## Provenance & pinning

- **Upstream:** <https://github.com/opencypher/openCypher>
- **Pinned tag:** `2024.3`
- **Pinned commit:** see [`PINNED_COMMIT`](PINNED_COMMIT)
- **Reproduced by:** [`scripts/tck/fetch.sh`](../../scripts/tck/fetch.sh)

To refresh or bump the corpus:

```bash
scripts/tck/fetch.sh 2024.3   # or a newer release tag
```

The script does a shallow, blobless, sparse clone of the upstream repo at the
given tag, copies `tck/features/` here, and records the exact commit in
`PINNED_COMMIT`. Bumping the tag is a deliberate, reviewed change (the scenario
count and pass-rate may move).

## License

The openCypher TCK is licensed under the **Apache License 2.0** — an approved,
license-clean family per
[`docs/process/open-source-guardrails.md`](../../docs/process/open-source-guardrails.md)
§5. The upstream [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE) are vendored here for
attribution. Each `.feature` file additionally carries the Apache-2.0 header and
the openCypher attribution notice inline.

These files are an industry-standard **conformance test suite**, not private or
proprietary data — vendoring them is consistent with the open-source guardrails
(license-clean, publicly published, redistribution permitted by Apache-2.0).

## What consumes these files

`tck-runner/` parses every `.feature` file here, executes each scenario against
the caerostris-db engine via a thin adapter, and emits a machine-readable
pass/pending/fail summary. See
[`docs/process/testing-and-benchmarks.md`](../../docs/process/testing-and-benchmarks.md)
§6 and the `tck-runner` crate docs.
