# Release Manifest

This document specifies Sigil release manifest format version 1. The normative
JSON shape is `language/spec/release-manifest.schema.json`.

## Release Asset Set

A complete release contains exactly five compiler archives:

1. `darwin-arm64`
2. `darwin-x64`
3. `linux-arm64`
4. `linux-x64`
5. `windows-x64`

It also contains `release-manifest.json` and `SHA256SUMS`.

## Manifest Shape

```json
{
  "artifacts": [],
  "compiler": {
    "capabilities": {},
    "version": "2026-07-10T16-00-00Z"
  },
  "formatVersion": 1,
  "source": {
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "repository": "inerte/sigil"
  },
  "version": "2026-07-10T16-00-00Z"
}
```

`compiler.capabilities` is the `data` object from the canonical
`sigil capabilities` response. Every platform binary must produce an identical
response before this value is selected.

Each artifact entry contains:

- `file`
- `archiveFormat`
- `sizeBytes`
- lowercase hexadecimal `sha256`
- `platform.id`, `os`, `architecture`, and Rust `target`

Artifact entries use the release asset order above. Object field order and
two-space JSON indentation are fixed by the generator, and the file ends with
one newline. `generatedAt` and other volatile values are not permitted.

## Invariants

- `formatVersion` is `1`.
- `version` and `compiler.version` are equal canonical UTC timestamps.
- `source.commit` is the full lowercase 40-character Git commit ID.
- Archive names contain the manifest version and platform ID.
- `tar.gz` is used for Unix archives and `zip` for Windows.
- Archive sizes and hashes are computed from the final uploaded bytes.
- Missing, duplicated, extra, or unsupported platform metadata is invalid.
- Platform capability responses must be identical.

## Checksums

`SHA256SUMS` contains the five archives and `release-manifest.json`, sorted by
filename. The manifest does not hash itself; the checksum file supplies its
digest.

Format version 1 does not define signatures, attestations, or reproducible-build
claims. Those guarantees require a new compatible field or a new manifest
format version, depending on their wire impact.
