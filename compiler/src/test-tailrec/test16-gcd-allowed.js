export function gcd(a, b) {
  return (() => {
  const __match = b;
  if (__match === 0) {
    return a;
  }
  else if (true) {
    const b = __match;
    return gcd(b, (a % b));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function main() {
  return gcd(48, 18);
}

