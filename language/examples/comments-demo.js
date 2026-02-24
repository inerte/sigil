export function factorial(n) {
  return (() => {
  const __match = n;
  if (__match === 0) {
    return 1;
  }
  else if (__match === 1) {
    return 1;
  }
  else if (true) {
    const n = __match;
    return (n * factorial((n - 1)));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function main() {
  return factorial(5);
}

