λhanoi(n:ℤ,from:𝕊,to:𝕊,aux:𝕊)→𝕊≡n{
  1→"Move disk from "+from+" to "+to+"\n"|
  n→hanoi(n-1,from,aux,to)+
    "Move disk from "+from+" to "+to+"\n"+
    hanoi(n-1,aux,to,from)
}

λmain()→𝕊=hanoi(3,"A","C","B")
