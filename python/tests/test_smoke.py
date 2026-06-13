"""Smoke tests for the caerostris PyO3 extension module.

These prove the FFI substrate (T-0030, rubric Cat. 8): the compiled module is
importable, a trivial exported function is callable from Python, and Rust
failures surface as a *typed* Python exception rather than a naked
``RuntimeError`` string. The real query/ingest API arrives in T-0031; this only
establishes the build + exception-mapping pattern.

Run with the extension built into the active environment::

    maturin develop -m python/Cargo.toml
    pytest python/tests
"""

import re

import caerostris


def test_module_is_importable() -> None:
    """The compiled extension imports under the package name ``caerostris``."""
    assert caerostris.__name__ == "caerostris"


def test_version_matches_semver() -> None:
    """``version()`` returns the crate version string (semver-shaped)."""
    version = caerostris.version()
    assert isinstance(version, str)
    assert re.fullmatch(r"\d+\.\d+\.\d+", version), version


def test_version_matches_package_dunder() -> None:
    """The exported ``__version__`` agrees with ``version()``."""
    assert caerostris.__version__ == caerostris.version()


def test_rust_panic_surfaces_as_typed_exception() -> None:
    """A Rust-side failure raises ``caerostris.CaerostrisError``.

    This pins the exception-mapping pattern that T-0031 builds on: engine
    failures must be a dedicated, catchable exception type — never a bare
    ``RuntimeError`` carrying an opaque string. ``_panic_for_testing`` exists
    solely to exercise this boundary.
    """
    assert issubclass(caerostris.CaerostrisError, Exception)
    with pytest.raises(caerostris.CaerostrisError):
        caerostris._panic_for_testing()


# pytest is imported late so the module-import smoke test fails with a clear
# ``ModuleNotFoundError: caerostris`` (the thing under test) rather than an
# unrelated import error, when run before the extension is built.
import pytest  # noqa: E402
