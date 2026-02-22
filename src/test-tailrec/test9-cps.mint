// TEST: Continuation Passing Style (CPS)
// EXPECTED: ❌ BLOCKED - "returns a function type"

λfactorial(n:ℤ)→λ(ℤ)→ℤ≡n{
  0→λacc→acc|
  n→λacc→factorial(n-1)(n*acc)
}

λmain()→ℤ=factorial(5)(1)
