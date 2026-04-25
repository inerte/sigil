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

if [[ -n "${SIGIL_BIN:-}" ]]; then
  sigil_cmd=("$SIGIL_BIN")
else
  sigil_cmd=(cargo run -q -p sigil-cli --no-default-features --)
fi

npm install --prefix language/runtime/node --registry=https://registry.npmjs.org

archive_base="sigil-${version}-darwin-arm64"
archive_stage_dir="$tmp_dir/$archive_base"
archive_extract_dir="$tmp_dir/extracted"
homebrew_prefix_dir="$tmp_dir/homebrew-prefix"

mkdir -p "$archive_stage_dir/runtime" "$archive_extract_dir" "$homebrew_prefix_dir/share/sigil"
cp -R "language/runtime/node" "$archive_stage_dir/runtime/node"
tar -C "$tmp_dir" -czf "$tmp_dir/$archive_base.tar.gz" "$archive_base"
tar -C "$archive_extract_dir" -xzf "$tmp_dir/$archive_base.tar.gz"
"$repo_root/tools/checkBundledNodeRuntime.sh" "$archive_extract_dir/$archive_base"
cp -R "$archive_extract_dir/$archive_base/runtime" "$homebrew_prefix_dir/share/sigil/runtime"
"$repo_root/tools/checkBundledNodeRuntime.sh" "$homebrew_prefix_dir"

"${sigil_cmd[@]}" test projects/homebrewPackaging/tests --env release

sigilHomebrewVersion="$version" \
sigilHomebrewRepo="inerte/sigil" \
sigilHomebrewSha256SumsPath="$checksums_path" \
sigilHomebrewOutputPath="$tmp_dir/sigil.rb" \
  "${sigil_cmd[@]}" run projects/homebrewPackaging/src/main.sigil --env release

diff -u "$expected_path" "$tmp_dir/sigil.rb"

if command -v ruby >/dev/null 2>&1; then
  ruby -c "$tmp_dir/sigil.rb"
fi
