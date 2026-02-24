⟦ Test that accumulator patterns are still blocked ⟧

λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{
  0→acc|
  n→factorial(n-1,n*acc)
}

λmain()→ℤ=factorial(5,1)
