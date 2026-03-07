#!/bin/bash

# Compile all .sigil files in the repo
# Stops on first compilation error

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color
SIGIL="language/compiler/target/debug/sigil"

COMPILED=0
FAILED=0

echo "═══════════════════════════════════════════════════════════"
echo "  Building Rust compiler"
echo "═══════════════════════════════════════════════════════════"
echo ""

if ! cargo build --quiet --manifest-path language/compiler/Cargo.toml -p sigil-cli 2>&1; then
  echo -e "${RED}Failed to build Rust compiler${NC}"
  exit 1
fi

echo -e "${GREEN}Rust compiler built successfully${NC}"
echo ""

echo "═══════════════════════════════════════════════════════════"
echo "  Compiling all .sigil files in repository"
echo "═══════════════════════════════════════════════════════════"
echo ""

# Find all .sigil files (both .sigil and .lib.sigil)
while IFS= read -r file; do
  echo -n "Compiling $(basename $file)... "

  if output=$("$SIGIL" compile "$file" 2>&1); then
    echo -e "${GREEN}✓${NC}"
    ((COMPILED++))
  else
    echo -e "${RED}✗ FAILED${NC}"
    echo ""
    echo "File: $file"
    echo ""
    echo "Error:"
    echo "$output" | jq -r '.error.message' 2>/dev/null || echo "$output"
    echo ""
    ((FAILED++))

    # Stop on first error
    echo "═══════════════════════════════════════════════════════════"
    echo -e "${RED}Stopped at first error${NC}"
    echo "Compiled: $COMPILED files"
    echo "Failed: $file"
    echo "═══════════════════════════════════════════════════════════"
    exit 1
  fi
done < <(find . \
  \( -path "*/.git" -o -path "*/target" -o -path "*/node_modules" -o -path "*/.local" \) -prune \
  -o -name "*.sigil" -type f -print | sort)

echo ""
echo "═══════════════════════════════════════════════════════════"
echo -e "${GREEN}All files compiled successfully!${NC}"
echo "Total: $COMPILED files"
echo "═══════════════════════════════════════════════════════════"
