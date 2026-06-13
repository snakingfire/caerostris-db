//! PyO3 embedded bindings for caerostris-db (EPIC-007, rubric Cat. 8).
//!
//! This crate compiles to a `cdylib` that [maturin] packages as the importable
//! `caerostris` Python extension module. It is intentionally a *separate*
//! workspace member from the engine crate so PyO3 and its build-time dependency
//! tree never enter the engine binary or its cold-start path.
//!
//! **Status (T-0030):** scaffold only. It exports:
//!
//! - [`version`] — the crate version string, plus a `__version__` module
//!   attribute, proving a Rust function is callable from Python.
//! - [`CaerostrisError`] — the typed Python exception that *all* engine
//!   failures map to. The real query/ingest API (T-0031) reuses this; FFI
//!   callers `except caerostris.CaerostrisError` rather than matching on a
//!   naked `RuntimeError` string.
//! - `_panic_for_testing` — exercises the panic→typed-exception boundary so the
//!   mapping pattern is covered by a test from day one. Underscore-prefixed and
//!   documented as test-only; it is not part of the public API.
//!
//! [maturin]: https://www.maturin.rs/

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    caerostris,
    CaerostrisError,
    PyException,
    "Base exception for all caerostris-db engine failures surfaced to Python.\n\n\
     Rust-side errors and caught panics are converted to this type so Python \
     callers can `except caerostris.CaerostrisError` instead of matching on a \
     bare `RuntimeError` string. T-0031's query/ingest API maps its domain \
     errors onto this same type."
);

/// The crate version, sourced from `Cargo.toml` at compile time.
///
/// Callable from Python as `caerostris.version()`; also exposed as the
/// `caerostris.__version__` module attribute.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Run `f`, converting any Rust panic into a [`CaerostrisError`] Python
/// exception instead of letting it unwind across the FFI boundary (where PyO3
/// would otherwise raise a generic `pyo3_runtime.PanicException`).
///
/// This is the canonical wrapper for fallible engine calls: T-0031 routes its
/// query/ingest entry points through it (or an `Result`-returning equivalent)
/// so Python always sees a typed, catchable exception.
fn map_panic_to_exception<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> PyResult<T> {
    match std::panic::catch_unwind(f) {
        Ok(value) => Ok(value),
        Err(payload) => {
            // `payload` is a `Box<dyn Any + Send>`; pass the inner `&dyn Any`
            // (via `as_ref`) so `panic_message` downcasts the *payload value*,
            // not the box itself.
            let message = panic_message(payload.as_ref());
            Err(CaerostrisError::new_err(format!(
                "caerostris engine panicked: {message}"
            )))
        }
    }
}

/// Extract a human-readable message from a panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

/// Test-only: trigger a Rust panic and surface it as a [`CaerostrisError`].
///
/// Exists solely so a pytest can assert the panic→typed-exception mapping. Not
/// part of the supported API (underscore-prefixed); do not call it from real
/// code.
#[pyfunction]
fn _panic_for_testing() -> PyResult<()> {
    map_panic_to_exception(|| panic!("intentional panic from _panic_for_testing"))
}

/// The `caerostris` Python module definition.
///
/// `#[pymodule]` wires the exported functions, the `__version__` attribute, and
/// the [`CaerostrisError`] exception type into the importable module.
#[pymodule]
fn caerostris(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    module.add_function(wrap_pyfunction!(_panic_for_testing, module)?)?;
    module.add("__version__", version())?;
    module.add("CaerostrisError", py.get_type::<CaerostrisError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_the_crate_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn map_panic_passes_through_ok_values() {
        let result = map_panic_to_exception(|| 21 * 2);
        assert_eq!(result.expect("non-panicking closure must be Ok"), 42);
    }

    #[test]
    fn map_panic_converts_str_payload() {
        let err = map_panic_to_exception::<()>(|| panic!("boom"))
            .expect_err("a panicking closure must be Err");
        Python::attach(|py| {
            assert!(err.is_instance_of::<CaerostrisError>(py));
            assert!(
                err.to_string().contains("boom"),
                "panic message lost: {err}"
            );
        });
    }

    #[test]
    fn map_panic_converts_string_payload() {
        let err = map_panic_to_exception::<()>(|| panic!("{}", String::from("dynamic")))
            .expect_err("a panicking closure must be Err");
        assert!(err.to_string().contains("dynamic"));
    }

    #[test]
    fn panic_message_handles_non_string_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(42_i32);
        assert_eq!(panic_message(payload.as_ref()), "unknown panic payload");
    }
}
