λhelper(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→helper(n-1,n*acc)}
λfactorial(n:ℤ)→ℤ=helper(n,1)
λdummy()→ℤ=helper(1,1)
λmain()→ℤ=factorial(5)
