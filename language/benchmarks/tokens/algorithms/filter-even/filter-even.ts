function isEven(x: number): boolean {
  return x % 2 === 0;
}

function filterEven(xs: number[]): number[] {
  return xs.filter(isEven);
}

function main(): number[] {
  return filterEven([1, 2, 3, 4, 5, 6]);
}
