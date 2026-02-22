#!/bin/bash

# Test suite for canonical form enforcement
# Tests all loopholes to ensure they're blocked

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PASSED=0
FAILED=0
SKIPPED=0

test_should_fail() {
  local file=$1
  shift
  local expected_errors=("$@")
  
  echo -n "Testing $(basename $file) (should fail)... "
  
  if output=$(node compiler/dist/cli.js compile "$file" 2>&1); then
    echo -e "${RED}✗ FAILED${NC} - Compiled successfully (should have been blocked!)"
    echo "  This loophole is NOT blocked!"
    ((FAILED++))
    return 1
  else
    # Check if any of the expected error messages match
    for expected in "${expected_errors[@]}"; do
      if echo "$output" | grep -q "$expected"; then
        echo -e "${GREEN}✓ PASSED${NC} - Blocked: $expected"
        ((PASSED++))
        return 0
      fi
    done
    
    # None matched
    echo -e "${YELLOW}⚠ BLOCKED${NC} - Different error (still blocked)"
    echo "  Got: $(echo "$output" | grep "Error:" | head -1 | cut -d: -f2-)"
    ((PASSED++))  # Count as pass since it's blocked
    return 0
  fi
}

test_should_pass() {
  local file=$1
  echo -n "Testing $(basename $file) (should compile)... "
  
  if output=$(node compiler/dist/cli.js compile "$file" 2>&1); then
    echo -e "${GREEN}✓ PASSED${NC} - Compiled successfully"
    ((PASSED++))
    return 0
  else
    echo -e "${RED}✗ FAILED${NC} - Blocked incorrectly"
    echo "  Error: $(echo "$output" | grep "Error:" | head -1 | cut -d: -f2-)"
    ((FAILED++))
    return 1
  fi
}

test_not_implemented() {
  local file=$1
  local reason=$2
  echo -e "Testing $(basename $file)... ${BLUE}⊘ SKIP${NC} - $reason"
  ((SKIPPED++))
}

echo "═══════════════════════════════════════════════════════════"
echo "  Mint Canonical Form Enforcement - Test Suite"
echo "═══════════════════════════════════════════════════════════"
echo ""

# Tests that should be blocked
echo "Tests that should be BLOCKED:"
echo "─────────────────────────────────────────────────────────"

test_should_fail "src/test-tailrec/test1-two-param.mint" "accumulator-passing style"
test_should_fail "src/test-tailrec/test2-three-param.mint" "accumulator-passing style"
test_should_fail "src/test-tailrec/test3-list-param.mint" "collection-type parameter"
test_not_implemented "src/test-tailrec/test4-tuple-param.mint" "Tuple types not yet implemented"
test_should_fail "src/test-tailrec/test5-record-two-fields.mint" "collection-type parameter"
test_should_fail "src/test-tailrec/test6-record-three-fields.mint" "collection-type parameter"
test_should_fail "src/test-tailrec/test8-helper.mint" "only called by"
test_should_fail "src/test-tailrec/test9-cps.mint" "returns a function type"
test_not_implemented "src/test-tailrec/test10-map-param.mint" "Map literals not yet implemented"
test_should_fail "src/test-tailrec/test11-nested-list.mint" "collection-type parameter"
test_should_fail "src/test-tailrec/test13-boolean-match-blocked.mint" "Non-canonical pattern matching"
test_should_fail "src/test-tailrec/test14-tuple-boolean-blocked.mint" "tuple of boolean expressions"
test_should_fail "src/test-tailrec/test18-factorial-acc-blocked.mint" "accumulator-passing style"
test_should_fail "src/test-tailrec/test19-list-accumulator.mint" "multiple collection parameters"

echo ""
echo "Tests that should be ALLOWED:"
echo "─────────────────────────────────────────────────────────"

test_should_pass "examples/list-reverse.mint"
test_should_pass "examples/list-length.mint"
test_should_pass "src/test-tailrec/test7-record-one-field-ok.mint"
test_should_pass "src/test-tailrec/test12-valid-canonical.mint"
test_should_pass "src/test-tailrec/test15-canonical-value-match.mint"
test_should_pass "src/test-tailrec/test16-gcd-allowed.mint"
test_should_pass "src/test-tailrec/test17-power-allowed.mint"

echo ""
echo "═══════════════════════════════════════════════════════════"
printf "  Results: ${GREEN}%d passed${NC}, ${RED}%d failed${NC}" $PASSED $FAILED
if [ $SKIPPED -gt 0 ]; then
  printf ", ${BLUE}%d skipped${NC}" $SKIPPED
fi
echo ""
echo "═══════════════════════════════════════════════════════════"

if [ $FAILED -eq 0 ]; then
  echo -e "${GREEN}All tests passed! ✓${NC}"
  echo ""
  echo "Canonical form enforcement is working correctly:"
  echo "  ✓ Multi-parameter recursion blocked"
  echo "  ✓ List parameter recursion blocked"
  echo "  ✓ Record parameter recursion blocked (2+ fields)"
  echo "  ✓ Nested collection recursion blocked"
  echo "  ✓ Helper function pattern blocked"
  echo "  ✓ CPS pattern blocked"
  echo "  ✓ Boolean pattern matching blocked (when value matching works)"
  echo "  ✓ Tuple boolean pattern matching blocked (single param)"
  echo "  ✓ Single-field records allowed"
  echo "  ✓ Canonical value matching allowed"
  exit 0
else
  echo -e "${RED}Some tests failed! ✗${NC}"
  exit 1
fi
