λrange(n:ℤ)→[ℤ]≡n{0→[]|n→[n,.range(n-1)]}
λmultiply(acc:ℤ,x:ℤ)→ℤ=acc*x
λfold(fn:λ(ℤ,ℤ)→ℤ,init:ℤ,list:[ℤ])→ℤ≡list{[]→init|[x,.xs]→fold(fn,fn(init,x),xs)}
λfactorial(n:ℤ)→ℤ=fold(multiply,1,range(n))
λmain()→ℤ=factorial(5)
