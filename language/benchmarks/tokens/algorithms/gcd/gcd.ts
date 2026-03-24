function gcd(a: number, b: number): number {
  if (b === 0) return a;
  return gcd(b, a % b);
}

function main(): number {
  return gcd(48, 18);
}
