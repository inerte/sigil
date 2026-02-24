export function reverse(lst) {
  return (() => {
  const __match = lst;
  if (__match.length === 0) {
    return [];
  }
  else if (__match.length >= 1) {
    const x = __match[0]; const xs = __match.slice(1);
    return reverse(xs).concat([].concat(x));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function main() {
  return reverse([].concat([1], [2], [3], [4], [5]));
}

