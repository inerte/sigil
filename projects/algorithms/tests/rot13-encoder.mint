i src/rot13-encoder

test "rot13 encodes hello world" {
  src/rot13-encoder.rot13(["H","e","l","l","o",","," ","W","o","r","l","d","!"])= "Uryyb, Jbeyq!"
}

test "rot13 preserves punctuation" {
  src/rot13-encoder.rot13(["!",".",","," "])="!., "
}
