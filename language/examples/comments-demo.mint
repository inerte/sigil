⟦ Factorial Function with Comments
   This demonstrates how to use multi-line comments in Mint.
   Comments use white square brackets and can appear anywhere. ⟧

λfactorial(n:ℤ)→ℤ≡n{
  0→1|  ⟦ base case: 0! = 1 ⟧
  1→1|  ⟦ base case: 1! = 1 ⟧
  n→n*⟦ multiply n by (n-1)! ⟧factorial(n-1)
}

⟦ Main function demonstrates:
   - Inline comments mid-expression
   - Multi-line explanatory comments
   - Comments don't affect execution ⟧
λmain()→ℤ=factorial(⟦ calculate 5! ⟧5)
