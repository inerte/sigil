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
  sigil_binary="$SIGIL_BIN"
else
  cargo build --quiet -p sigil-cli --no-default-features
  sigil_binary="$repo_root/target/debug/sigil"
fi

sigil_cmd=("$sigil_binary")

npm ci --prefix language/runtime/node --registry=https://registry.npmjs.org

archive_base="sigil-${version}-darwin-arm64"
archive_stage_dir="$tmp_dir/$archive_base"
archive_extract_dir="$tmp_dir/extracted"
homebrew_prefix_dir="$tmp_dir/homebrew-prefix"

mkdir -p \
  "$archive_stage_dir/runtime" \
  "$archive_stage_dir/language" \
  "$archive_extract_dir" \
  "$homebrew_prefix_dir/bin" \
  "$homebrew_prefix_dir/share/sigil"
cp "$sigil_binary" "$archive_stage_dir/sigil"
cp -R "language/runtime/node" "$archive_stage_dir/runtime/node"
for dir in core stdlib world test; do
  cp -R "language/$dir" "$archive_stage_dir/language/$dir"
done
tar -C "$tmp_dir" -czf "$tmp_dir/$archive_base.tar.gz" "$archive_base"
tar -C "$archive_extract_dir" -xzf "$tmp_dir/$archive_base.tar.gz"
"$repo_root/tools/checkBundledNodeRuntime.sh" "$archive_extract_dir/$archive_base"
bash "$repo_root/tools/checkBundledLanguageRoot.sh" "$archive_extract_dir/$archive_base"
cp "$archive_extract_dir/$archive_base/sigil" "$homebrew_prefix_dir/bin/sigil"
cp -R "$archive_extract_dir/$archive_base/runtime" "$homebrew_prefix_dir/share/sigil/runtime"
cp -R "$archive_extract_dir/$archive_base/language" "$homebrew_prefix_dir/share/sigil/language"
"$repo_root/tools/checkBundledNodeRuntime.sh" "$homebrew_prefix_dir"
bash "$repo_root/tools/checkBundledLanguageRoot.sh" "$homebrew_prefix_dir"

smoke_dir="$tmp_dir/homebrew-smoke"
mkdir -p "$smoke_dir"
pushd "$smoke_dir" >/dev/null
"$homebrew_prefix_dir/bin/sigil" init >/dev/null
cat > src/main.sigil <<'EOF'
λmain()=>Int=1+1
EOF
cat > tests/basic.sigil <<'EOF'
λmain()=>Unit=()

test "adds" {
  1+1=2
}
EOF
"$homebrew_prefix_dir/bin/sigil" inspect codegen src/main.sigil >/dev/null
"$homebrew_prefix_dir/bin/sigil" compile . >/dev/null
"$homebrew_prefix_dir/bin/sigil" test >/dev/null
popd >/dev/null

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
