//! An [`ObjectStore`] backed by a real S3-compatible bucket (MinIO in the
//! demo, production S3 in the field), implemented by shelling out to the
//! `aws s3api` CLI.
//!
//! ## Why the CLI rather than an SDK crate?
//!
//! This is the demo keystone — it proves the engine's *object-storage-native*
//! claim by persisting the graph as real objects in a real bucket. We delegate
//! to the already-present `aws` CLI instead of pulling an HTTP/S3 SDK crate so
//! the demo:
//!
//! - adds **zero new Rust dependencies** (nothing to license-vet, no large
//!   transitive tree dragged into the engine's cold-start build / lockfile),
//! - works **offline** against the swarm's local MinIO with no crate fetch, and
//! - is dead simple to reason about: every method is one `aws s3api …` call.
//!
//! When the durable storage cascade (T-0007/T-0008/T-0009) lands its own
//! object writers, this adapter is superseded; it lives in the demo lane only.
//!
//! ## Configuration
//!
//! [`S3CliStore`] is told the endpoint URL, bucket, and a key prefix; AWS
//! credentials and region come from the process environment (the swarm exports
//! them via `.project/env/local.env`). Keys handed to the [`ObjectStore`] trait
//! are namespaced under the configured prefix so multiple work items share one
//! MinIO without collision.

use std::process::Command;

use super::{ObjectStore, StoreError};

/// An [`ObjectStore`] that persists objects in an S3-compatible bucket via the
/// `aws s3api` command-line tool.
///
/// Every operation is a single `aws s3api` invocation against the configured
/// `--endpoint-url`. Credentials/region are read from the environment by the
/// CLI itself, matching how the swarm provisions MinIO.
#[derive(Debug, Clone)]
pub struct S3CliStore {
    /// S3 endpoint URL, e.g. `http://127.0.0.1:9000` for local MinIO.
    endpoint: String,
    /// Target bucket name.
    bucket: String,
    /// Key prefix applied to every object (kept with its trailing `/`, if any).
    prefix: String,
}

impl S3CliStore {
    /// Create a store targeting `bucket` at `endpoint`, namespacing all keys
    /// under `prefix`.
    ///
    /// `prefix` may be empty (no namespacing) or end in `/`; it is joined to
    /// caller keys verbatim, so pass `"demo/"` to get `demo/<key>`.
    #[must_use]
    pub fn new(
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            bucket: bucket.into(),
            prefix: prefix.into(),
        }
    }

    /// Build an [`S3CliStore`] targeting an explicit bucket/prefix, reading only
    /// the endpoint from the environment.
    ///
    /// Reads `CAEROSTRIS_S3_ENDPOINT` (required). The bucket and prefix are
    /// passed explicitly.
    ///
    /// # Errors
    /// Returns [`StoreError::Backend`] if `CAEROSTRIS_S3_ENDPOINT` is unset.
    pub fn from_env(
        bucket: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Result<Self, StoreError> {
        let endpoint = std::env::var("CAEROSTRIS_S3_ENDPOINT").map_err(|_| {
            StoreError::Backend("CAEROSTRIS_S3_ENDPOINT is not set; run scripts/env/up.sh".into())
        })?;
        Ok(Self::new(endpoint, bucket, prefix))
    }

    /// Build an [`S3CliStore`] entirely from the swarm's environment variables,
    /// as exported by `scripts/env/up.sh` and `scripts/env/bucket.sh`:
    ///
    /// - `CAEROSTRIS_S3_ENDPOINT` (required) — the S3/MinIO endpoint URL.
    /// - `CAEROSTRIS_S3_BUCKET` (required) — the per-work-item bucket.
    /// - `CAEROSTRIS_S3_PREFIX` (optional, default empty) — the per-run prefix.
    ///
    /// # Errors
    /// Returns [`StoreError::Backend`] if endpoint or bucket is unset.
    pub fn from_swarm_env() -> Result<Self, StoreError> {
        let endpoint = std::env::var("CAEROSTRIS_S3_ENDPOINT").map_err(|_| {
            StoreError::Backend("CAEROSTRIS_S3_ENDPOINT is not set; run scripts/env/up.sh".into())
        })?;
        let bucket = std::env::var("CAEROSTRIS_S3_BUCKET").map_err(|_| {
            StoreError::Backend(
                "CAEROSTRIS_S3_BUCKET is not set; run eval \"$(scripts/env/bucket.sh demo)\""
                    .into(),
            )
        })?;
        let prefix = std::env::var("CAEROSTRIS_S3_PREFIX").unwrap_or_default();
        Ok(Self::new(endpoint, bucket, prefix))
    }

    /// The fully-qualified object key for a caller-supplied `key`.
    fn full_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    /// Strip the configured prefix off a listed key so callers see the same key
    /// space they `put` into.
    fn strip_prefix<'k>(&self, full: &'k str) -> &'k str {
        full.strip_prefix(&self.prefix).unwrap_or(full)
    }

    /// Run `aws s3api <args…>` with our `--endpoint-url`, returning the captured
    /// process output. The caller inspects `status` and `stdout`/`stderr`.
    fn run(&self, args: &[&str]) -> Result<std::process::Output, StoreError> {
        let output = Command::new("aws")
            .arg("--endpoint-url")
            .arg(&self.endpoint)
            .arg("s3api")
            .args(args)
            .output()
            .map_err(|e| StoreError::Backend(format!("failed to spawn aws CLI: {e}")))?;
        Ok(output)
    }

    /// Create the bucket if it does not already exist (idempotent).
    ///
    /// MinIO returns success on create and a `BucketAlreadyOwnedByYou`-style
    /// error otherwise; both are treated as "the bucket now exists".
    ///
    /// # Errors
    /// Returns [`StoreError::Backend`] only if the CLI cannot be spawned.
    pub fn ensure_bucket(&self) -> Result<(), StoreError> {
        // create-bucket failing because it already exists is fine; we only fail
        // if the CLI itself could not run.
        let _ = self.run(&["create-bucket", "--bucket", &self.bucket])?;
        Ok(())
    }
}

impl ObjectStore for S3CliStore {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        let full = self.full_key(key);
        // aws s3api put-object reads the body from a file path; write the bytes
        // to a temp file in the OS temp dir, then upload it.
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "caero-s3-put-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| StoreError::Backend(format!("temp write failed: {e}")))?;
        let body_arg = tmp.to_string_lossy().to_string();
        let result = self.run(&[
            "put-object",
            "--bucket",
            &self.bucket,
            "--key",
            &full,
            "--body",
            &body_arg,
        ]);
        // Always clean up the temp file regardless of outcome.
        let _ = std::fs::remove_file(&tmp);
        let output = result?;
        if !output.status.success() {
            return Err(StoreError::Backend(format!(
                "put-object {full:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        let full = self.full_key(key);
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "caero-s3-get-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let dest = tmp.to_string_lossy().to_string();
        let output = self.run(&[
            "get-object",
            "--bucket",
            &self.bucket,
            "--key",
            &full,
            &dest,
        ])?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp);
            let stderr = String::from_utf8_lossy(&output.stderr);
            // MinIO/S3 report a missing key as NoSuchKey / 404.
            if stderr.contains("NoSuchKey")
                || stderr.contains("Not Found")
                || stderr.contains("404")
            {
                return Err(StoreError::NotFound(key.to_owned()));
            }
            return Err(StoreError::Backend(format!(
                "get-object {full:?} failed: {}",
                stderr.trim()
            )));
        }
        let bytes = std::fs::read(&tmp)
            .map_err(|e| StoreError::Backend(format!("temp read failed: {e}")))?;
        let _ = std::fs::remove_file(&tmp);
        Ok(bytes)
    }

    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        if start > end {
            return Err(StoreError::RangeOutOfBounds {
                key: key.to_owned(),
                object_len: 0,
                start,
                end,
            });
        }
        // The trait's `end` is exclusive; the HTTP/S3 Range header is inclusive,
        // so request `bytes=start-(end-1)`. An empty range short-circuits.
        if start == end {
            // Confirm the object exists so an empty range on a missing key still
            // surfaces NotFound, matching MemoryStore semantics.
            let _ = self.get(key)?;
            return Ok(Vec::new());
        }
        let full = self.full_key(key);
        let range = format!("bytes={}-{}", start, end - 1);
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "caero-s3-range-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let dest = tmp.to_string_lossy().to_string();
        let output = self.run(&[
            "get-object",
            "--bucket",
            &self.bucket,
            "--key",
            &full,
            "--range",
            &range,
            &dest,
        ])?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("NoSuchKey")
                || stderr.contains("Not Found")
                || stderr.contains("404")
            {
                return Err(StoreError::NotFound(key.to_owned()));
            }
            // An out-of-range request comes back as InvalidRange / 416.
            if stderr.contains("InvalidRange") || stderr.contains("416") {
                return Err(StoreError::RangeOutOfBounds {
                    key: key.to_owned(),
                    object_len: 0,
                    start,
                    end,
                });
            }
            return Err(StoreError::Backend(format!(
                "get-object (range) {full:?} failed: {}",
                stderr.trim()
            )));
        }
        let bytes = std::fs::read(&tmp)
            .map_err(|e| StoreError::Backend(format!("temp read failed: {e}")))?;
        let _ = std::fs::remove_file(&tmp);
        Ok(bytes)
    }

    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        let full = self.full_key(key);
        let output = self.run(&["delete-object", "--bucket", &self.bucket, "--key", &full])?;
        // S3 delete is idempotent: deleting a missing key still returns success.
        if !output.status.success() {
            return Err(StoreError::Backend(format!(
                "delete-object {full:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let full_prefix = self.full_key(prefix);
        let output = self.run(&[
            "list-objects-v2",
            "--bucket",
            &self.bucket,
            "--prefix",
            &full_prefix,
            "--query",
            "Contents[].Key",
            "--output",
            "text",
        ])?;
        if !output.status.success() {
            return Err(StoreError::Backend(format!(
                "list-objects-v2 prefix {full_prefix:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        // An empty listing comes back as "None" (the JMESPath null) or empty.
        if trimmed.is_empty() || trimmed == "None" {
            return Ok(Vec::new());
        }
        // `--output text` separates keys with tabs and/or newlines.
        let mut keys: Vec<String> = trimmed
            .split(['\t', '\n'])
            .filter(|s| !s.is_empty())
            .map(|full| self.strip_prefix(full).to_owned())
            .collect();
        keys.sort();
        Ok(keys)
    }
}

/// A short, process-local random-ish suffix for temp file names. Avoids a
/// dependency on the `rand` crate for what is only collision-avoidance within
/// one process's temp files.
fn rand_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_key_applies_prefix() {
        let s = S3CliStore::new("http://x", "bucket", "demo/");
        assert_eq!(s.full_key("nodes/0.json"), "demo/nodes/0.json");
    }

    #[test]
    fn full_key_empty_prefix_is_identity() {
        let s = S3CliStore::new("http://x", "bucket", "");
        assert_eq!(s.full_key("k"), "k");
    }

    #[test]
    fn strip_prefix_removes_namespace() {
        let s = S3CliStore::new("http://x", "bucket", "demo/");
        assert_eq!(s.strip_prefix("demo/nodes/0.json"), "nodes/0.json");
        // A key without the prefix is returned unchanged.
        assert_eq!(s.strip_prefix("other/x"), "other/x");
    }

    #[test]
    fn from_env_errors_without_endpoint() {
        // Ensure the var is unset for this assertion.
        // SAFETY: single-threaded unit test; no other thread reads the env here.
        unsafe {
            std::env::remove_var("CAEROSTRIS_S3_ENDPOINT");
        }
        let err = S3CliStore::from_env("b", "p/").unwrap_err();
        assert!(matches!(err, StoreError::Backend(_)));
    }

    #[test]
    fn rand_suffix_is_nonzero() {
        // Smoke test: the suffix helper produces a value (used only for temp
        // file uniqueness).
        assert!(rand_suffix() > 0);
    }
}
