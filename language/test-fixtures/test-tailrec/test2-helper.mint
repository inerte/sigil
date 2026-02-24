// TEST: Helper function with accumulator
// EXPECTED: ❌ BLOCKED - Accumulator detection still works (but not because it's a helper)

λfact_helper(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→fact_helper(n-1,n*acc)}
λfactorial(n:ℤ)→ℤ=fact_helper(n,1)
λmain()→ℤ=factorial(5)
