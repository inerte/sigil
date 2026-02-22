// TEST: List parameter (encoding multiple values)
// EXPECTED: ❌ BLOCKED - "collection-type parameter"

λfactorial(state:[ℤ])→ℤ≡state{
  [0,acc]→acc|
  [n,acc]→factorial([n-1,n*acc])
}

λmain()→ℤ=factorial([5,1])
