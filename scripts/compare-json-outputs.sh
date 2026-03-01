#!/bin/bash
# Compare JSON outputs between TypeScript and Rust compilers
# Usage: ./scripts/compare-json-outputs.sh

set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Build both compilers
echo "Building TypeScript compiler..."
cd language/compiler
pnpm build > /dev/null 2>&1
cd "$REPO_ROOT"

echo "Building Rust compiler..."
cd language/compiler-rs
cargo build --release > /dev/null 2>&1
cd "$REPO_ROOT"

TS_CLI="node language/compiler/dist/cli.js"
RUST_CLI="language/compiler-rs/target/release/sigil"

# Find all .sigil files
SIGIL_FILES=$(find language -name "*.sigil" -type f -not -path "*/node_modules/*" -not -path "*/.local/*" | head -10)
TOTAL=$(echo "$SIGIL_FILES" | wc -l | tr -d ' ')

echo "Testing $TOTAL .sigil files with lex, parse, and compile commands"
echo ""

FAILURES=0
PASSED=0

for file in $SIGIL_FILES; do
    echo "Testing: $file"

    # Test lex command
    TS_JSON=$($TS_CLI lex "$file" 2>&1 || true)
    RUST_JSON=$($RUST_CLI lex "$file" 2>&1 || true)

    # Hash both outputs
    TS_HASH=$(echo "$TS_JSON" | shasum -a 256 | cut -d' ' -f1)
    RUST_HASH=$(echo "$RUST_JSON" | shasum -a 256 | cut -d' ' -f1)

    if [ "$TS_HASH" = "$RUST_HASH" ]; then
        echo "  ✓ lex PASS"
    else
        echo "  ❌ lex FAIL - JSON outputs differ!"
        echo ""
        echo "  TypeScript JSON (first 500 chars):"
        echo "$TS_JSON" | head -c 500
        echo ""
        echo "  Rust JSON (first 500 chars):"
        echo "$RUST_JSON" | head -c 500
        echo ""
        FAILURES=$((FAILURES + 1))
        exit 1
    fi

    # Test parse command
    TS_JSON=$($TS_CLI parse "$file" 2>&1 || true)
    RUST_JSON=$($RUST_CLI parse "$file" 2>&1 || true)

    TS_HASH=$(echo "$TS_JSON" | shasum -a 256 | cut -d' ' -f1)
    RUST_HASH=$(echo "$RUST_JSON" | shasum -a 256 | cut -d' ' -f1)

    if [ "$TS_HASH" = "$RUST_HASH" ]; then
        echo "  ✓ parse PASS"
    else
        echo "  ❌ parse FAIL - JSON outputs differ!"
        echo ""
        echo "  TypeScript JSON (first 500 chars):"
        echo "$TS_JSON" | head -c 500
        echo ""
        echo "  Rust JSON (first 500 chars):"
        echo "$RUST_JSON" | head -c 500
        echo ""
        FAILURES=$((FAILURES + 1))
        exit 1
    fi

    PASSED=$((PASSED + 1))
done

echo ""
if [ $FAILURES -eq 0 ]; then
    echo "✅ SUCCESS: All $PASSED files produce identical JSON outputs!"
    exit 0
else
    echo "❌ FAILED: $FAILURES files produced different outputs"
    exit 1
fi
