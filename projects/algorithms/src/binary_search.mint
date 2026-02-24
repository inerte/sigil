λbinary_search(xs:[ℤ],target:ℤ,low:ℤ,high:ℤ)→ℤ=
  ≡(high<low,xs[(((low+high)-((low+high)%2))/2)]=target,xs[(((low+high)-((low+high)%2))/2)]<target){
    (⊤,_,_)→-1|
    (⊥,⊤,_)→(((low+high)-((low+high)%2))/2)|
    (⊥,⊥,⊤)→binary_search(xs,target,(((low+high)-((low+high)%2))/2)+1,high)|
    (⊥,⊥,⊥)→binary_search(xs,target,low,(((low+high)-((low+high)%2))/2)-1)
  }

λmain()→ℤ=binary_search([1,3,5,7,9,11,13,15,17,19],13,0,9)
