e console

λshowInt(n:ℤ)→𝕊=""+n

λcountLower(n:ℤ)→𝕊≡n{0→"no more bottles"|1→"1 bottle"|n→showInt(n)++" bottles"}

λcountUpper(n:ℤ)→𝕊≡n{0→"No more bottles"|1→"1 bottle"|n→showInt(n)++" bottles"}

λaction(n:ℤ)→𝕊≡n{0→"Go to the store and buy some more"|1→"Take it down and pass it around"|n→"Take one down and pass it around"}

λnextCount(n:ℤ)→𝕊≡n{0→countLower(99)|n→countLower(n-1)}

λverse(n:ℤ)→𝕊=countUpper(n)++" of beer on the wall, "++countLower(n)++" of beer.\n"++action(n)++", "++nextCount(n)++" of beer on the wall."

λsong(n:ℤ)→𝕊≡n{0→verse(0)|n→verse(n)++"\n\n"++song(n-1)}

λmain()→𝕌=console.log(song(99))
