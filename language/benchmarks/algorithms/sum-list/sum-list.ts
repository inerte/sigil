function sum(xs: number[]): number {
  return xs.reduce((a, x) => a + x, 0);
}

function main(): number {
  return sum([1, 2, 3, 4, 5]);
}
