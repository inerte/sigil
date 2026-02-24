// TEST: Tuple boolean matching on single parameter
// EXPECTED: ❌ BLOCKED - "tuple of boolean expressions on single parameter"

λfib(n:ℤ)→ℤ≡(n=0,n=1){
  (⊤,_)→0|
  (_,⊤)→1|
  (⊥,⊥)→fib(n-1)+fib(n-2)
}

λmain()→ℤ=fib(7)
