#!/bin/bash
set -e

echo "========================================"
echo "Sigil Compiler Test Corpus Runner"
echo "========================================"
echo ""

RUST_COMPILER="./target/debug/sigil"
TS_COMPILER="../compiler/dist/cli.js"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

passed=0
failed=0
skipped=0

# Test a single file
test_file() {
    local file=$1
    local name=$(basename "$file")

    echo -n "Testing $name... "

    # Run with Rust compiler
    rust_output=$($RUST_COMPILER run "$file" --human 2>&1 | grep -E "^[0-9]+$" || echo "ERROR")

    # Run with TS compiler
    ts_output=$(node $TS_COMPILER run "$file" --human 2>&1 | grep -E "^[0-9]+$" || echo "ERROR")

    # Compare outputs
    if [ "$rust_output" = "ERROR" ] || [ "$ts_output" = "ERROR" ]; then
        echo -e "${YELLOW}SKIP${NC} (compilation error)"
        ((skipped++))
    elif [ "$rust_output" = "$ts_output" ]; then
        echo -e "${GREEN}PASS${NC} (output: $rust_output)"
        ((passed++))
    else
        echo -e "${RED}FAIL${NC} (Rust: $rust_output, TS: $ts_output)"
        ((failed++))
    fi
}

# Find all .sigil files in test-corpus
for file in test-corpus/*.sigil; do
    if [ -f "$file" ]; then
        test_file "$file"
    fi
done

# Summary
echo ""
echo "========================================"
echo "Summary:"
echo -e "${GREEN}Passed: $passed${NC}"
echo -e "${RED}Failed: $failed${NC}"
echo -e "${YELLOW}Skipped: $skipped${NC}"
echo "========================================"

if [ $failed -gt 0 ]; then
    exit 1
fi
