// TEST: Two parameters (classic accumulator pattern)
// EXPECTED: ❌ BLOCKED - "has 2 parameters"

λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{
  0→acc|
  n→factorial(n-1,n*acc)
}

λmain()→ℤ=factorial(5,1)
