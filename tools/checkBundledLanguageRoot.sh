#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <bundle-root>" >&2
  exit 1
fi

bundle_root="$1"

language_dir=""
for candidate in \
  "$bundle_root/language" \
  "$bundle_root/share/sigil/language"
do
  if [[ -d "$candidate" ]]; then
    language_dir="$candidate"
    break
  fi
done

if [[ -z "$language_dir" ]]; then
  echo "failed to locate bundled language root under '$bundle_root'" >&2
  exit 1
fi

required_files=(
  "core/prelude.lib.sigil"
  "stdlib/path.lib.sigil"
  "world/runtime.lib.sigil"
  "test/check/file.lib.sigil"
  "test/observe/file.lib.sigil"
)

for required in "${required_files[@]}"; do
  if [[ ! -f "$language_dir/$required" ]]; then
    echo "missing required bundled language file: $language_dir/$required" >&2
    find "$language_dir" -maxdepth 3 \( -type f -o -type d \) | sort >&2 || true
    exit 1
  fi
done

echo "validated bundled language root under $language_dir"
