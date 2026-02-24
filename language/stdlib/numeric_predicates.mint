⟦ Numeric Range Predicates

   Standard predicates for numeric validation and range checking.
   Part of Mint standard library - canonical implementations only.
⟧

⟦ Check if value is positive ⟧
λis_positive(x:ℤ)→𝔹=x>0

⟦ Check if value is negative ⟧
λis_negative(x:ℤ)→𝔹=x<0

⟦ Check if value is zero ⟧
λis_zero(x:ℤ)→𝔹=x=0

⟦ Check if value is non-negative (≥0) ⟧
λis_non_negative(x:ℤ)→𝔹=x≥0

⟦ Check if value is even ⟧
λis_even(x:ℤ)→𝔹=(x%2)=0

⟦ Check if value is odd ⟧
λis_odd(x:ℤ)→𝔹=¬(is_even(x))

⟦ Check if value is in range [min, max] (inclusive) ⟧
λin_range(x:ℤ,min:ℤ,max:ℤ)→𝔹=in_range_helper(x,min,max)

λin_range_helper(x:ℤ,min:ℤ,max:ℤ)→𝔹≡(x≥min){
  ⊤→x≤max|
  ⊥→⊥
}

⟦ Check if value is prime ⟧
λis_prime(n:ℤ)→𝔹≡n{
  0→⊥|
  1→⊥|
  n→is_prime_helper(n,2)
}

⟦ Helper function for prime checking ⟧
λis_prime_helper(n:ℤ,divisor:ℤ)→𝔹≡(divisor*divisor>n){
  ⊤→⊤|
  ⊥→≡(n%divisor≠0){
    ⊤→is_prime_helper(n,divisor+1)|
    ⊥→⊥
  }
}
