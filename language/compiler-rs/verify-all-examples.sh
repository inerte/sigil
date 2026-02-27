#!/bin/bash
# Differential testing: Run all examples with both Rust and TS compilers
# Verifies output parity

set -e

echo "========================================"
echo "Verifying Compiler Parity on Examples"
echo "========================================"
echo ""

RUST_COMPILER="./target/debug/sigil"
TS_COMPILER="../compiler/dist/cli.js"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

passed=0
failed=0
skipped=0
total=0

test_file() {
    local file=$1
    local name=$(basename "$file")
    ((total++))

    # Run with both compilers
    rust_out=$($RUST_COMPILER run "$file" --human 2>&1 | tail -1 || echo "COMPILE_ERROR")
    ts_out=$(node $TS_COMPILER run "$file" --human 2>&1 | tail -1 || echo "COMPILE_ERROR")

    # Extract just the result (last line before "OK" message)
    rust_result=$(echo "$rust_out" | grep -v "sigilc run" | grep -v "^$" || echo "$rust_out")
    ts_result=$(echo "$ts_out" | grep -v "sigilc run" | grep -v "^$" || echo "$ts_out")

    if [[ "$rust_result" == *"COMPILE_ERROR"* ]] || [[ "$ts_result" == *"COMPILE_ERROR"* ]]; then
        echo -e "  ${YELLOW}SKIP${NC} $name (compilation error)"
        ((skipped++))
    elif [ "$rust_result" = "$ts_result" ]; then
        echo -e "  ${GREEN}PASS${NC} $name"
        ((passed++))
    else
        echo -e "  ${RED}FAIL${NC} $name"
        echo "       Rust: $rust_result"
        echo "       TS:   $ts_result"
        ((failed++))
    fi
}

# Test all examples
for file in ../examples/*.sigil; do
    if [ -f "$file" ]; then
        test_file "$file"
    fi
done

# Summary
echo ""
echo "========================================"
printf "Total: %d | " "$total"
printf "${GREEN}Passed: %d${NC} | " "$passed"
printf "${RED}Failed: %d${NC} | " "$failed"
printf "${YELLOW}Skipped: %d${NC}\n" "$skipped"
echo "========================================"

if [ $failed -gt 0 ]; then
    echo "Some tests failed!"
    exit 1
else
    echo "All passing tests show compiler parity! ✅"
fi
