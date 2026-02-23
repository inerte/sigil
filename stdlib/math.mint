⟦
  Mint Standard Library - Math Operations

  Pure Mint implementations - canonical recursive forms.
  NO FFI - demonstrates what Mint can do natively.
⟧

⟦ ========================================================================
   BASIC COMPARISONS
   ======================================================================== ⟧

⟦ Minimum of two numbers ⟧
λmin(a:ℤ,b:ℤ)→ℤ≡a<b{⊤→a|⊥→b}

⟦ Maximum of two numbers ⟧
λmax(a:ℤ,b:ℤ)→ℤ≡a>b{⊤→a|⊥→b}

⟦ Clamp value between min and max ⟧
λclamp(x:ℤ,lo:ℤ,hi:ℤ)→ℤ=max(lo,min(x,hi))

⟦ ========================================================================
   POWER
   ======================================================================== ⟧

⟦ Integer power (exponentiation) ⟧
λpow(base:ℤ,exp:ℤ)→ℤ≡exp{
  0→1|
  exp→base*pow(base,exp-1)
}

⟦ ========================================================================
   DIVISIBILITY AND PRIMES
   ======================================================================== ⟧

⟦ Check if n is divisible by d ⟧
λdivisible(n:ℤ,d:ℤ)→𝔹=n%d=0

⟦ Greatest common divisor (Euclidean algorithm) ⟧
λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}

⟦ Check if prime (trial division up to sqrt) ⟧
λprime_helper(n:ℤ,d:ℤ)→𝔹≡d*d>n{
  ⊤→⊤|
  ⊥→≡divisible(n,d){⊤→⊥|⊥→prime_helper(n,d+1)}
}

λis_prime(n:ℤ)→𝔹≡n{
  0→⊥|
  1→⊥|
  2→⊤|
  n→prime_helper(n,2)
}

⟦ ========================================================================
   SEQUENCES AND SUMMATIONS
   ======================================================================== ⟧

⟦ Sum of integers from 1 to n ⟧
λsum_to(n:ℤ)→ℤ≡n{0→0|n→n+sum_to(n-1)}

⟦ Sum of integers from a to b ⟧
λsum_range(a:ℤ,b:ℤ)→ℤ≡a>b{⊤→0|⊥→a+sum_range(a+1,b)}

⟦ Product of integers from 1 to n (factorial) ⟧
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}

⟦ Fibonacci number ⟧
λfib(n:ℤ)→ℤ≡n{0→0|1→1|n→fib(n-1)+fib(n-2)}
