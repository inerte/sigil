// TEST: Helper function pattern (single param version)
// EXPECTED: ✅ ALLOWED - Utility functions are now allowed (helper ban removed)

λhelper(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*helper(n-1)
}

λfactorial(n:ℤ)→ℤ=helper(n)

λmain()→ℤ=factorial(5)
