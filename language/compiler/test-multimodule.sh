#!/bin/bash
set -e

echo "========================================"
echo "Multi-Module Integration Tests"
echo "========================================"
echo ""

# Clean up from previous runs
rm -rf test-project

# Test 1: Simple multi-file project
echo "Test 1: Multi-file project (src::utils)"
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

cat > test-project/src/utils.lib.sigil << 'EOF'
λdouble(x:Int)=>Int=x*2
λtriple(x:Int)=>Int=x*3
EOF

cat > test-project/src/main.sigil << 'EOF'
i src::utils
λmain()=>Int=src::utils.double(21)
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/main.sigil"
cd test-project
../target/debug/sigil run src/main.sigil
cd ..
echo ""

# Test 2: Multiple imports
echo "Test 2: Multiple imports in one file"
echo "--------------------------------------"
cat > test-project/src/math.lib.sigil << 'EOF'
λadd(x:Int,y:Int)=>Int=x+y
λsubtract(x:Int,y:Int)=>Int=x-y
EOF

cat > test-project/src/calc.sigil << 'EOF'
i src::math
i src::utils
λmain()=>Int=src::math.add(src::utils.double(10),src::utils.triple(5))
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/calc.sigil"
cd test-project
../target/debug/sigil run src/calc.sigil
cd ..
echo ""

# Test 3: Nested module dependencies
echo "Test 3: Transitive dependencies (A imports B imports C)"
echo "---------------------------------------------------------"
cat > test-project/src/base.lib.sigil << 'EOF'
λincrement(x:Int)=>Int=x+1
EOF

cat > test-project/src/derived.lib.sigil << 'EOF'
i src::base
λaddTwo(x:Int)=>Int=src::base.increment(src::base.increment(x))
EOF

cat > test-project/src/app.sigil << 'EOF'
i src::derived
λmain()=>Int=src::derived.addTwo(5)
EOF

echo "Running: cd test-project && ../target/debug/sigil run src/app.sigil"
cd test-project
../target/debug/sigil run src/app.sigil
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
