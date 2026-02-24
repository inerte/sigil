λfactHelper(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factHelper(n-1)}
λfactorial(n:ℤ)→ℤ=factHelper(n)
λmain()→ℤ=factorial(5)
