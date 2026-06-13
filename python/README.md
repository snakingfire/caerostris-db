# caerostris (Python bindings)

PyO3 + maturin embedded bindings for [caerostris-db](../README.md), a graph
database engine on commodity object storage.

**Status:** scaffold (T-0030). Today the module exports a `version()` function
and the `CaerostrisError` exception type that all engine failures map to. The
open/attach/query/ingest API lands in T-0031.

## Build & test

From the repository root, with the dev toolchain available
(`rust-toolchain.toml` / the Nix shell) and a Python ≥ 3.9 environment:

```bash
# Build the extension into the active environment and run the smoke tests.
maturin develop -m python/Cargo.toml
pytest python/tests

# Build a wheel (lands under target/wheels/, gitignored).
maturin build -m python/Cargo.toml --release
```

```python
import caerostris

caerostris.version()        # -> "0.0.0"
caerostris.__version__      # same value, as a module attribute

try:
    ...                     # an engine call that fails
except caerostris.CaerostrisError as exc:
    ...                     # typed, catchable — never a bare RuntimeError
```

See [`docs/adr/0004-python-bindings-pyo3-maturin.md`](../docs/adr/0004-python-bindings-pyo3-maturin.md)
for the build/packaging decisions.
