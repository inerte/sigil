// TEST: Boolean matching when value matching works
// EXPECTED: ❌ BLOCKED - "Non-canonical pattern matching"

λsum(n:ℤ)→ℤ≡(n=0){
  ⊤→0|
  ⊥→n+sum(n-1)
}

λmain()→ℤ=sum(5)
