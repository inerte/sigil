# Releasing Sigil

Sigil releases are automatic promotions of commits that pass the required
quality gate on `main`. A release is not defined by a successful source build.
It is defined by five validated archives, their canonical manifest, and their
checksums.

## Required Quality Gate

Run the same gate locally that CI uses:

```bash
pnpm sigil:quality
```

The gate checks Rust formatting and compiler tests, CLI and release schemas,
repo invariants, every registered first-party Sigil suite, integration tests,
website builds, and source-tree packaging. Every check in this command is a
release blocker.

Project test directories are registered in
`projects/testAllRunner/quality-gate.json`. The `test-registration` repo-audit
check rejects unregistered, stale, duplicate, or unexplained entries. An
exemption records that a suite runs through another required phase; it does not
make that suite optional.

## Candidate Construction

After CI succeeds for a push to `main`, the release workflow derives the
canonical timestamp version from that commit and builds these required targets:

- `darwin-arm64`
- `darwin-x64`
- `linux-arm64`
- `linux-x64`
- `windows-x64`

Each archive contains the executable, bundled language root, Node runtime
helpers and platform-native dependencies, license, and release readme. Windows
also contains its required Z3 runtime DLLs.

The workflow extracts every archive outside the checkout. It verifies bundled
files and then uses the extracted executable for `--version`, `capabilities`,
`init`, `inspect codegen`, `compile`, `test`, and `run`. It also compiles and
executes a PTY-backed test fixture so a source-tree fallback cannot hide a
missing runtime bundle.

## Manifest and Checksums

All five binaries must return the same canonical `sigil capabilities` response.
Only then does the workflow generate:

- `release-manifest.json`, conforming to
  `language/spec/release-manifest.schema.json`
- `SHA256SUMS`, covering the manifest and every platform archive

The manifest contains no generation time. Its source identity, compiler
capabilities, platform order, archive sizes, and hashes are deterministic for a
given candidate artifact set.

Verify a downloaded release with:

```bash
sha256sum -c SHA256SUMS
```

On macOS, use `shasum -a 256` against the individual entries if GNU
`sha256sum` is not installed.

## Publication

No tag or public release is created until candidate validation finishes. The
workflow creates the GitHub release for the exact commit and uploads the five
archives, manifest, and checksums together.

Published assets are immutable by workflow policy:

- the workflow never uses asset clobbering
- an exact rerun is a no-op
- a matching version with different commit or metadata is a hard failure
- an incomplete draft may be deleted and rebuilt before publication
- concurrent older releases cannot replace a newer release as `Latest`

GitHub archives are the authoritative distribution. Website deployment and
Homebrew tap publication do not determine whether a GitHub release is valid.

## Homebrew Channel

Publishing a GitHub release triggers the independent Homebrew workflow. It
downloads the published assets, verifies `SHA256SUMS`, generates the formula
with the released Sigil binary, performs a real formula install and test, and
only then updates the tap.

The workflow can be retried manually with a published version. A stale retry
never replaces a newer tap formula. If the deploy key is unavailable, the
GitHub release remains valid and the tap stays at its last verified version.

## Rollback

Before publication, discard a failed candidate or draft and rerun the workflow.
Do not publish a partial artifact set.

After publication, do not overwrite or delete the released tag or assets.
Revert or fix the fault on `main`; the successful corrective commit receives a
new timestamp version. Until then, restore the tap to the last known-good
formula if Homebrew users would otherwise receive the affected release.

Document the reason in the affected release notes and in the corrective commit.
The old manifest must remain available so existing downloads stay auditable.

## Trust Boundary

Release manifest v1 provides deterministic metadata and SHA-256 integrity. It
does not claim bit-for-bit reproducible builds, artifact signing, or build
provenance attestations.
