#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

checksums_path="packaging/homebrew/SHA256SUMS"
expected_path="packaging/homebrew/Formula/sigil.rb"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

version="$(
  python3 - <<'PY'
import re
line=open("packaging/homebrew/SHA256SUMS").read().splitlines()[0]
match=re.search(r"sigil-(.*)-darwin-arm64\.tar\.gz", line)
print(match.group(1) if match else "")
PY
)"

if [[ -z "$version" ]]; then
  echo "could not derive release version from $checksums_path" >&2
  exit 1
fi

sigil_bin="language/compiler/target/debug/sigil"

"$sigil_bin" test projects/homebrewPackaging/tests

sigilHomebrewVersion="$version" \
sigilHomebrewRepo="inerte/sigil" \
sigilHomebrewSha256SumsPath="$checksums_path" \
sigilHomebrewOutputPath="$tmp_dir/sigil.rb" \
  "$sigil_bin" run projects/homebrewPackaging/src/main.sigil

diff -u "$expected_path" "$tmp_dir/sigil.rb"

if command -v ruby >/dev/null 2>&1; then
  ruby -c "$tmp_dir/sigil.rb"
fi
