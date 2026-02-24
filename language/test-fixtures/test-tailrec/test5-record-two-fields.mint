// TEST: Record type with 2 fields
// EXPECTED: ❌ BLOCKED - "collection-type parameter"

t State={n:ℤ,acc:ℤ}

λfactorial(state:State)→ℤ≡state.n{
  0→state.acc|
  n→factorial({n:n-1,acc:n*state.acc})
}

λmain()→ℤ=factorial({n:5,acc:1})
