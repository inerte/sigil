⟦ Built-in list operations demonstration ⟧

⟦ Map: [1,2,3] ↦ λx→x*2 ⟧
λdouble_all(xs:[ℤ])→[ℤ]=xs↦λx→x*2

⟦ Filter: [1,2,3,4,5] ⊳ λx→x>2 ⟧
λkeep_large(xs:[ℤ])→[ℤ]=xs⊳λx→x>2

⟦ Fold: [1,2,3] ⊕ λ(acc,x)→acc+x ⊕ 0 ⟧
λsum(xs:[ℤ])→ℤ=xs⊕(λ(acc,x)→acc+x)⊕0

⟦ Chaining: filter, then map ⟧
λfilter_then_map(xs:[ℤ])→[ℤ]=(xs⊳λx→x>0)↦λx→x*x

λmain()→ℤ=
  l nums=[1,2,3,4,5];
  sum(nums)
