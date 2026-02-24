// TEST: GCD with multi-param recursion (should COMPILE)
// EXPECTED: ✅ COMPILES - legitimate multi-param algorithm

λgcd(a:ℤ,b:ℤ)→ℤ≡b{
  0→a|
  b→gcd(b,a%b)
}

λmain()→ℤ=gcd(48,18)
