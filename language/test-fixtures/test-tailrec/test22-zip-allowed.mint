// TEST: Append two lists with multi-param recursion (should COMPILE)
// EXPECTED: ✅ COMPILES - both lists decompose (one to empty, other structural)

λappend(xs:[ℤ],ys:[ℤ])→[ℤ]≡xs{
  []→ys|
  [x,.rest]→[x,.append(rest,ys)]
}

λmain()→[ℤ]=append([1,2,3],[4,5,6])
