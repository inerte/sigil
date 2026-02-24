λgcd(a:ℤ,b:ℤ)→ℤ≡b{
  0→a|
  b→gcd(b,a%b)
}

λlcm(a:ℤ,b:ℤ)→ℤ=(a*b)/gcd(a,b)

λmain()→ℤ=gcd(48,18)
