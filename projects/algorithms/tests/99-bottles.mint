i src/bottles99

test "99 bottles verse 2" {
  src/bottles99.verse(2)="2 bottles of beer on the wall, 2 bottles of beer.\nTake one down and pass it around, 1 bottle of beer on the wall."
}

test "99 bottles verse 0" {
  src/bottles99.verse(0)="No more bottles of beer on the wall, no more bottles of beer.\nGo to the store and buy some more, 99 bottles of beer on the wall."
}
