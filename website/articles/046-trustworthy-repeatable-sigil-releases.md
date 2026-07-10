---
title: Trustworthy, Repeatable Sigil Releases
date: 2026-07-10
author: Sigil Language Team
slug: trustworthy-repeatable-sigil-releases
---

# Trustworthy, Repeatable Sigil Releases

Sigil's source tests had grown substantially, but the release process did not
apply all of them as one promotion decision. CI ran compiler tests and one
application project. A separate aggregate runner knew about more language,
project, documentation, website, and packaging checks, but the release workflow
did not use it as its gate.

The workflow also created a Git tag before its platform archives finished
building. Unix archives were extracted and inspected, while the Windows archive
did not contain the same language and Node runtime payload. A rerun could
replace existing release assets. Those are individually understandable
implementation choices, but together they made a release less trustworthy than
the source commit it represented.

Sigil now treats release construction as a promotion pipeline with one required
quality decision and one complete artifact contract.

## One Required Gate

The canonical contributor and CI command is:

```bash
pnpm sigil:quality
```

It runs Rust formatting and compiler tests, machine-output and release schemas,
repo audit, language and integration suites, every registered project suite,
website builds, and packaging validation. These checks are all blockers. A
failure is not relabeled as advisory merely because it is inconvenient in a
release workflow.

Project suites live in a versioned registry. The repo audit compares that
registry with tracked `projects/*/tests/*.sigil` files. Adding a new project
test directory without registering it fails the gate. Removing a project while
leaving stale registration also fails. This converts test coverage from a
hard-coded convention into a checked repository invariant.

## The Archive Is the Product

Source-tree tests cannot establish that an installed compiler works. The
release contract therefore requires five archives:

- macOS arm64 and x64
- Linux arm64 and x64
- Windows x64

Every archive has the same logical payload: compiler executable, language root,
runtime helpers and dependencies, license, and release readme. Windows also
ships the Z3 DLLs needed by its executable.

Each platform job extracts its archive outside the checkout and invokes that
extracted binary. The smoke sequence checks the version and canonical
capabilities response, initializes a project, inspects code generation,
compiles, tests, and runs code. A PTY-backed fixture exercises a surface that
depends on the packaged Node runtime. This prevents the source checkout from
silently satisfying a missing installed file.

## A Manifest Connects Source to Bytes

After all five jobs pass, the workflow compares their complete
`sigil capabilities` responses. A release cannot contain platform binaries that
disagree about commands, output formats, phases, or supported features.

The aggregator then writes `release-manifest.json`. Format version 1 records:

- the release version and exact Git commit
- the compiler version and capability data
- each platform ID, architecture, operating system, and target triple
- each archive's filename, format, byte size, and SHA-256 hash

Entries have one fixed order. Volatile generation timestamps are excluded.
Generating the manifest twice from the same artifact metadata produces the
same bytes. `SHA256SUMS` covers both the manifest and all five archives.

This is an integrity and traceability contract. It does not claim that builds
are bit-for-bit reproducible, and it does not introduce signing or provenance
attestations. Those are separate guarantees that should be added only when the
release process can state and test them precisely.

## Publication Happens Last

The workflow does not create a public tag or release until archive smoke tests,
capability comparison, manifest generation, schema validation, and checksum
verification have all passed.

Published assets are not overwritten. An exact rerun is a no-op; the same
version associated with different source or metadata is an error. An
incomplete draft may be discarded before publication, but a published release
remains available so existing downloads and manifests stay auditable.

If concurrent commits finish out of order, the older release may still be
published for its successful commit, but it cannot replace the newer version as
the repository's latest release.

## Homebrew Is a Distribution Channel

GitHub archives are authoritative. Homebrew publication begins only after a
GitHub release is published, in an independent workflow. That workflow
downloads the public artifacts, verifies their checksums, generates the formula
with the released Sigil binary, installs and tests the formula, and then updates
the tap.

This boundary matters operationally. A missing deploy key or temporary tap
failure does not invalidate a correct GitHub release. The channel can be
retried by version, and a stale retry cannot replace a newer formula.

## Rollback Means a New Release

Before publication, a failed candidate can be discarded. After publication,
rollback does not mean replacing bytes beneath an existing version. The faulty
change is reverted or corrected on `main`, the full gate runs again, and the
new successful commit receives a new timestamp version.

Homebrew can temporarily return to the last known-good formula while that
corrective release is prepared. The affected GitHub release remains intact,
with its manifest and checksums, so users and agents can still determine
exactly what was shipped.

Canonical source makes program meaning easier to reproduce. Canonical machine
output makes compiler decisions easier to consume. The release pipeline now
extends the same principle to the boundary where source becomes installed
software.
