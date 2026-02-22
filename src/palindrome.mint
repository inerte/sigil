λreverse(lst:[ℤ])→[ℤ]≡lst{
  []→[]|
  [x,.xs]→reverse(xs)++[x]
}

λisPalindrome(lst:[ℤ])→𝔹=lst=reverse(lst)

λmain()→𝔹=isPalindrome([1,2,3,2,1])
