λnextCollatz(n:ℤ)→ℤ≡n%2{
  0→(n)/2|
  _→3*n+1
}

λcollatz(n:ℤ)→[ℤ]≡n{
  1→[1]|
  n→[n]⧺collatz(nextCollatz(n))
}

λmain()→[ℤ]=collatz(13)
