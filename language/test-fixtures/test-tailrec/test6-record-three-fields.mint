// TEST: Record type with 3 fields
// EXPECTED: ❌ BLOCKED - "collection-type parameter"

t State={n:ℤ,acc:ℤ,count:ℤ}

λfactorial(state:State)→ℤ≡state.n{
  0→state.acc|
  n→factorial({n:n-1,acc:n*state.acc,count:state.count+1})
}

λmain()→ℤ=factorial({n:5,acc:1,count:0})
