#!/bin/bash
set -e

echo "========================================"
echo "Multi-Module Integration Tests"
echo "========================================"
echo ""

# Clean up from previous runs
rm -rf test-project

# Test 1: Simple multi-file project
echo "Test 1: Multi-file project (src⋅utils)"
echo "----------------------------------------"
mkdir -p test-project/src

# Create project config
cat > test-project/sigil.json << 'EOF'
{
  "layout": {
    "src": "src",
    "tests": "tests",
    "out": ".local"
  }
}
EOF

cat > test-project/src/utils.sigil << 'EOF'
export λ double(x: ℤ) → ℤ = x * 2
export λ triple(x: ℤ) → ℤ = x * 3
EOF

cat > test-project/src/main.sigil << 'EOF'
i src⋅utils
λ main() → ℤ = src⋅utils.double(21)
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/main.sigil --human"
cd test-project
../target/debug/sigil run src/main.sigil --human
cd ..
echo ""

# Test 2: Multiple imports
echo "Test 2: Multiple imports in one file"
echo "--------------------------------------"
cat > test-project/src/math.sigil << 'EOF'
export λ add(x: ℤ, y: ℤ) → ℤ = x + y
export λ subtract(x: ℤ, y: ℤ) → ℤ = x - y
EOF

cat > test-project/src/calc.sigil << 'EOF'
i src⋅utils
i src⋅math
λ main() → ℤ = src⋅math.add(src⋅utils.double(10), src⋅utils.triple(5))
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/calc.sigil --human"
cd test-project
../target/debug/sigil run src/calc.sigil --human
cd ..
echo ""

# Test 3: Nested module dependencies
echo "Test 3: Transitive dependencies (A imports B imports C)"
echo "---------------------------------------------------------"
cat > test-project/src/base.sigil << 'EOF'
export λ increment(x: ℤ) → ℤ = x + 1
EOF

cat > test-project/src/derived.sigil << 'EOF'
i src⋅base
export λ add_two(x: ℤ) → ℤ = src⋅base.increment(src⋅base.increment(x))
EOF

cat > test-project/src/app.sigil << 'EOF'
i src⋅derived
λ main() → ℤ = src⋅derived.add_two(5)
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/app.sigil --human"
cd test-project
../target/debug/sigil run src/app.sigil --human
cd ..
echo ""

# Clean up
echo "Cleaning up test files..."
rm -rf test-project
rm -rf .local

echo ""
echo "========================================"
echo "Integration tests complete!"
echo "========================================"
