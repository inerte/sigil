λlength(lst:[ℤ])→ℤ≡lst{
  []→0|
  [_,.xs]→1+length(xs)
}

λmain()→ℤ=length([1,2,3,4,5])
