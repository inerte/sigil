⟦ Practical Sum Types Example

Demonstrates sum types with concrete (non-generic) examples.

Note: Generic Option[T] and Result[T,E] work for pattern matching,
but generic utility functions require full generic type inference (not yet implemented).
This example shows the working pattern with concrete types.
⟧

⟦ Type declarations - concrete types for integers ⟧
t IntOption=IntSome(ℤ)|IntNone
t IntResult=IntOk(ℤ)|IntErr(𝕊)

⟦ Safe list head ⟧
λsafe_head(xs:[ℤ])→IntOption≡xs{
  []→IntNone()|
  [x,.rest]→IntSome(x)
}

⟦ Safe subtraction with validation ⟧
λsafe_subtract(num1:ℤ,num2:ℤ)→IntResult≡num2>10{
  ⊤→IntErr("second number too large")|
  ⊥→IntOk(num1-num2)
}

⟦ Extract value from IntOption with default ⟧
λget_or_zero(opt:IntOption)→ℤ≡opt{
  IntSome(x)→x|
  IntNone→0
}

⟦ Extract value from IntResult with fallback ⟧
λget_or_default(res:IntResult,fallback:ℤ)→ℤ≡res{
  IntOk(value)→value|
  IntErr(_)→fallback
}

⟦ Check if IntResult is ok ⟧
λis_ok(res:IntResult)→𝔹≡res{
  IntOk(_)→⊤|
  IntErr(_)→⊥
}

⟦ Main - demonstrate usage ⟧
λmain()→ℤ=get_or_zero(safe_head([1,2,3]))+get_or_default(safe_subtract(10,2),0)+get_or_default(safe_subtract(10,20),999)
