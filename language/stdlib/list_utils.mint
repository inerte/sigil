⟦ List Utility Functions

   Standard utility functions for list operations.
   Part of Mint standard library - canonical implementations only.
⟧

⟦ Get length of list ⟧
λlen(xs:[ℤ])→ℤ≡xs{
  []→0|
  [x,.rest]→1+len(rest)
}

⟦ Get first element of list (unsafe - crashes on empty) ⟧
λhead(xs:[ℤ])→ℤ≡xs{
  [x,.rest]→x
}

⟦ Get all elements except first (unsafe - crashes on empty) ⟧
λtail(xs:[ℤ])→[ℤ]≡xs{
  [x,.rest]→rest
}
