#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SIGIL="${REPO_ROOT}/language/compiler/target/debug/sigil"

if [[ ! -x "${SIGIL}" ]]; then
  cargo build --quiet --manifest-path "${REPO_ROOT}/language/compiler/Cargo.toml" -p sigil-cli
fi

cd "${REPO_ROOT}"
"${SIGIL}" run language/testHarnesses/src/canonicalHarness.sigil
