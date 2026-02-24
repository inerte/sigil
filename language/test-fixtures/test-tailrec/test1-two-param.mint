// TEST: Two parameters with accumulator pattern
// EXPECTED: ❌ BLOCKED - "accumulator-passing style"

λsum(n:ℤ,acc:ℤ)→ℤ≡n{
  0→acc|
  n→sum(n-1,acc+n)
}

λmain()→ℤ=sum(5,0)
