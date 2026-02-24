⟦ Test that helper functions are now allowed ⟧

⟦ Utility function used by multiple functions ⟧
λis_positive(x:ℤ)→𝔹=x>0

λprocess_a(x:ℤ)→𝕊≡is_positive(x){
  ⊤→"positive"|
  ⊥→"negative"
}

λprocess_b(y:ℤ)→𝕊≡is_positive(y){
  ⊤→"yes"|
  ⊥→"no"
}

λmain()→𝕊=process_a(50)++" and "++process_b(-5)
