⟦ List Validation Predicates

   Standard predicates for list validation and checking.
   Part of Mint standard library - canonical implementations only.
⟧

⟦ Check if list is sorted in ascending order ⟧
λsorted_asc(xs:[ℤ])→𝔹≡xs{
  []→⊤|
  [_]→⊤|
  [a,.tail]→sorted_asc_check(a,tail)
}

λsorted_asc_check(prev:ℤ,xs:[ℤ])→𝔹≡xs{
  []→⊤|
  [b,.rest]→≡(prev≤b){
    ⊤→sorted_asc_check(b,rest)|
    ⊥→⊥
  }
}

⟦ Check if list is sorted in descending order ⟧
λsorted_desc(xs:[ℤ])→𝔹≡xs{
  []→⊤|
  [_]→⊤|
  [a,.tail]→sorted_desc_check(a,tail)
}

λsorted_desc_check(prev:ℤ,xs:[ℤ])→𝔹≡xs{
  []→⊤|
  [b,.rest]→≡(prev≥b){
    ⊤→sorted_desc_check(b,rest)|
    ⊥→⊥
  }
}

⟦ Check if index is valid for list
   TODO: Requires len() function from stdlib ⟧

⟦ Check if list is empty ⟧
λis_empty(xs:[ℤ])→𝔹≡xs{
  []→⊤|
  _→⊥
}

⟦ Check if list is non-empty ⟧
λis_non_empty(xs:[ℤ])→𝔹≡xs{
  []→⊥|
  _→⊤
}

⟦ Check if all elements satisfy predicate ⟧
λall(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹≡xs{
  []→⊤|
  [x,.rest]→≡pred(x){
    ⊤→all(pred,rest)|
    ⊥→⊥
  }
}

⟦ Check if any element satisfies predicate ⟧
λany(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹≡xs{
  []→⊥|
  [x,.rest]→≡pred(x){
    ⊤→⊤|
    ⊥→any(pred,rest)
  }
}

⟦ Check if element is in list ⟧
λcontains(item:ℤ,xs:[ℤ])→𝔹≡xs{
  []→⊥|
  [x,.rest]→≡(x=item){
    ⊤→⊤|
    ⊥→contains(item,rest)
  }
}
