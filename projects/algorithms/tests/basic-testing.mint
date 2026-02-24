mockable λping()→!IO 𝕊="real"

test "pure boolean passes" {
  1+1=2
}

test "mockable function can be overridden" →!IO {
  with_mock(ping, λ()→!IO 𝕊="fake") {
    ping()="fake"
  }
}
