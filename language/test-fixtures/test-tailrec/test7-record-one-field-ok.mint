// TEST: Record type with 1 field (should be ALLOWED - not encoding multiple values)
// EXPECTED: ✅ COMPILES

t Wrapper={value:ℤ}

λfactorial(n:Wrapper)→ℤ≡n.value{
  0→1|
  1→1|
  v→v*factorial({value:v-1})
}

λmain()→ℤ=factorial({value:5})
