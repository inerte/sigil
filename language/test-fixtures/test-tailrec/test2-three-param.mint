// TEST: Three parameters
// EXPECTED: ❌ BLOCKED - "accumulator-passing style"

λfold_sum(xs:[ℤ],acc:ℤ,count:ℤ)→ℤ≡xs{
  []→acc|
  [x,.rest]→fold_sum(rest,acc+x,count+1)
}

λmain()→ℤ=fold_sum([1,2,3],0,0)
