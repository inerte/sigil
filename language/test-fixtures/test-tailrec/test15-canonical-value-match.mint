// TEST: Canonical value matching (should COMPILE)
// EXPECTED: ✅ COMPILES

λsum(n:ℤ)→ℤ≡n{
  0→0|
  n→n+sum(n-1)
}

λmain()→ℤ=sum(5)
