# Open-Source Guardrails — caerostris-db

> These are **non-negotiable**. Violating any rule in this document is a
> **P0 stop-the-line event**: the offending change must be reverted, the incident
> logged in `.project/decisions/`, and the swarm notified before work continues.
> No exception, no fast-path, no "it's just a test."

---

## 1. This is a public repository

Every commit, every file, every comment, every log output that touches this
repository is **publicly visible**. There is no private branch. There is no
staging area that is not public. Act accordingly at all times.

---

## 2. Never commit secrets or credentials

The following must **never** appear in any file committed to this repository:

- AWS access keys or secret keys (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
  `AWS_SESSION_TOKEN`)
- Any API key, token, or bearer credential for any service
- Passwords or passphrases
- Private keys (PEM, DER, SSH, GPG)
- OAuth client secrets
- Connection strings that include credentials
- Any file whose name ends in `.env`, `.pem`, `.key`, `.p12`, `.pfx`, `.pkcs12`

### Enforcement

- **gitleaks** runs in the pre-commit hook. It will block a commit if it detects
  a credential pattern. Do not skip hooks (`--no-verify`).
- `.env*` files are gitignored. Do not remove this rule.
- If gitleaks fires a false positive, add the specific pattern to the `.gitleaks.toml`
  allowlist with a comment explaining why it is safe — do not disable gitleaks.
- If you accidentally commit a secret, treat it as **already compromised**: revoke
  it immediately, rotate it, then rewrite git history with `git filter-repo` (not
  `git filter-branch`). File a P0 incident entry in `.project/decisions/`.

### Where real credentials go

AWS credentials for real S3 access come from:
- Environment variables set outside the repo (`AWS_PROFILE`, `AWS_ACCESS_KEY_ID` +
  `AWS_SECRET_ACCESS_KEY` from a secrets manager)
- EC2/ECS instance roles (no credentials in the environment at all)
- AWS CLI named profiles (`~/.aws/credentials`) on developer machines

**Never** from a file in this repository. Never from a hardcoded string in the code.
When writing code that needs credentials, use the `aws-config` crate's
environment/profile/instance-role chain — it picks up the right credentials
automatically without the code knowing where they came from.

---

## 3. Never commit private or proprietary data

- No graph datasets, unless they are hand-crafted tiny fixtures (≤ a few KB) that
  we authored ourselves for testing.
- No data derived from datasets whose license does not permit redistribution.
- No personally identifiable information (PII).
- No proprietary third-party data.

Large datasets are stored locally under `data/` (gitignored) and on S3 (not in
the repo). See [`datasets.md`](datasets.md).

---

## 4. Build artifacts are not committed

`target/`, compiled binaries, Python wheels, and coverage reports are gitignored.
Do not remove these gitignore rules. If you need to share a build artifact, build
it from a tagged commit — do not commit it.

---

## 5. License hygiene for dependencies

Every dependency added to `Cargo.toml` (or `pyproject.toml` for Python tooling)
must carry a license that is **compatible with this project's license** and
permissive enough for open-source distribution.

### Approved license families

- MIT
- Apache-2.0
- BSD-2-Clause, BSD-3-Clause
- ISC
- MPL-2.0 (Mozilla Public License) — compatible, with file-level copyleft
- CC0-1.0 (for data/assets, not code)
- Unlicense / Public Domain
- Zlib
- Unicode-3.0 (Unicode License v3) — permissive, OSI-approved; the license under
  which Unicode data tables ship (e.g. `unicode-ident`, real SPDX `(MIT OR
  Apache-2.0) AND Unicode-3.0`). No copyleft / no source-availability clause. See
  `.project/decisions/0034-unicode-3-0-approved-license.md` (BUG-0023).

### Licenses that require steering sign-off before adding

- **GPL-2.0, GPL-3.0, LGPL-*, AGPL-*:** copyleft; may infect the binary or
  require open-sourcing linking code. Flag to steering before adding.
- **SSPL, BUSL, CC BY-NC:** non-commercial or source-available clauses; not
  suitable for a general open-source project without careful review.
- **Unknown / no license:** treat as all-rights-reserved; do not use.

### Protocol

1. Before adding a new dependency: check `cargo deny` or run
   `cargo license --avoid-build-deps` to audit the license.
2. If the license is in the approved list: proceed.
3. If the license requires sign-off: file a design spike, get steering approval,
   record the decision in `.project/decisions/` before adding.

```bash
# Audit all dependency licenses:
cargo deny check licenses

# List all dependency licenses:
cargo license --avoid-build-deps
```

---

## 6. License hygiene for datasets

Every dataset used for testing or benchmarking must have its license verified
**before** ingest. See [`datasets.md`](datasets.md) for the full protocol.

The `dataset-scout` agent must:
1. Read the license text at the source.
2. Confirm permissible use for software testing.
3. Record the verification in `.project/decisions/NNNN-dataset-license-<name>.md`.

A dataset without a recorded license verification is **blocked from ingest**.

---

## 7. No destructive git operations without explicit per-action authorization

The following git operations are **blocked by default** and require explicit,
per-action authorization from a human (Jonas) before execution:

- `git reset --hard`
- `git push --force` or `git push --force-with-lease` to `main`
- `git branch -D` (force-delete a branch)
- `git rebase --onto` that rewrites `main` history
- `git filter-repo` or `git filter-branch` (used only for secret removal P0s,
  with authorization)

If you believe a destructive operation is necessary, **stop, file a decision entry
explaining what you want to do and why**, and wait for human authorization. Do not
proceed autonomously.

Non-destructive operations (`git rebase` on a feature branch, `git commit --amend`
before pushing, `git tag`) do not require authorization but must not rewrite
already-pushed history on `main`.

---

## 8. Violation response protocol

If any guardrail is violated (by any agent, including an automated script):

1. **Stop.** Do not land further changes until the violation is addressed.
2. **Revert** the offending commit(s) from `main` immediately.
3. **Revoke and rotate** any leaked credentials immediately, before doing anything
   else.
4. **File a P0 incident** in `.project/decisions/NNNN-incident-<date>.md`:
   - What happened.
   - Which agent/script was responsible.
   - What was exposed or damaged.
   - What was done to remediate.
   - What process change prevents recurrence.
5. **Notify the swarm** by updating the board with a P0 blocker linked to the
   incident entry.
6. Resume work only after remediation is complete and the incident is filed.

---

## Quick reference card

| Rule | Violation = |
|---|---|
| Commit a secret / credential | P0 incident + rotate immediately |
| Commit raw graph data (>tiny fixture) | P0 incident |
| Commit a build artifact or binary | P0 incident |
| Add a GPL/AGPL dep without sign-off | P0 block until sign-off |
| Use a dataset without license verification | P0 block until verified |
| Run `git reset --hard` / `push --force` on main without authorization | P0 incident |
| Disable gitleaks (`--no-verify`) | P0 incident |
