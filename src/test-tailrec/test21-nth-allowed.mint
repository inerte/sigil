// TEST: Nth element with multi-param recursion (should COMPILE)
// EXPECTED: ✅ COMPILES - both params decompose in parallel

λnth(list:[ℤ],n:ℤ)→ℤ≡(list,n){
  ([x,.xs],0)→x|
  ([x,.xs],n)→nth(xs,n-1)
}

λmain()→ℤ=nth([10,20,30,40],2)
