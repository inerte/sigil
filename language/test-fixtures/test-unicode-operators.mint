⟦ Test Unicode operators in typechecker ⟧

e console

⟦ Test ≤ and ≥ ⟧
λtest_le(a:ℤ,b:ℤ)→𝔹=a≤b
λtest_ge(a:ℤ,b:ℤ)→𝔹=a≥b

⟦ Test ≠ ⟧
λtest_ne(a:ℤ,b:ℤ)→𝔹=a≠b

⟦ Test ∧ and ∨ ⟧
λtest_and(a:𝔹,b:𝔹)→𝔹=a∧b
λtest_or(a:𝔹,b:𝔹)→𝔹=a∨b

λmain()→𝕌=console.log("All tests passed: " ++ test_le(5,10) ++ ", " ++ test_ge(10,5) ++ ", " ++ test_ne(5,10) ++ ", " ++ ¬test_and(⊤,⊥) ++ ", " ++ test_or(⊤,⊥))
