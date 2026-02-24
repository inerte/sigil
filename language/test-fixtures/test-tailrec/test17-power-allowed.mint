// TEST: Power with multi-param recursion (should COMPILE)
// EXPECTED: ✅ COMPILES - legitimate multi-param algorithm

λpower(base:ℤ,exp:ℤ)→ℤ≡exp{
  0→1|
  exp→base*power(base,exp-1)
}

λmain()→ℤ=power(2,10)
