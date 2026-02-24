export function fibonacci(n) {
  return (() => {
  const __match = n;
  if (__match === 0) {
    return 0;
  }
  else if (__match === 1) {
    return 1;
  }
  else if (true) {
    const n = __match;
    return (fibonacci((n - 1)) + fibonacci((n - 2)));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

