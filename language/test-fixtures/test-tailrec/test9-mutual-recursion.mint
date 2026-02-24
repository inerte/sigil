λfactA(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factB(n-1)}
λfactB(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factA(n-1)}
λmain()→ℤ=factA(5)
