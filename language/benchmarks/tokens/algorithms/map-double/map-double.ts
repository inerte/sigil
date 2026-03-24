function double(x: number): number {
  return x * 2;
}

function mapDouble(xs: number[]): number[] {
  return xs.map(double);
}

function main(): number[] {
  return mapDouble([1, 2, 3, 4, 5]);
}
