// TEST: Valid canonical form (should COMPILE)
// EXPECTED: ✅ COMPILES

λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

λmain()→ℤ=factorial(5)
