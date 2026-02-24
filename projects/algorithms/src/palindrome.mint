λreverse(lst:[ℤ])→[ℤ]≡lst{
  []→[]|
  [x,.xs]→reverse(xs)⧺[x]
}

λmain()→[ℤ]=reverse([1,2,3,2,1])
