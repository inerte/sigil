// TEST: List as accumulator parameter
// EXPECTED: ❌ BLOCKED - "accumulator pattern"

λreverse_acc(lst:[ℤ],acc:[ℤ])→[ℤ]≡lst{
  []→acc|
  [x,.xs]→reverse_acc(xs,[x,.acc])
}

λmain()→[ℤ]=reverse_acc([1,2,3],[])
