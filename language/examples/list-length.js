export function length(lst) {
  return (() => {
  const __match = lst;
  if (__match.length === 0) {
    return 0;
  }
  else if (__match.length >= 1) {
    const xs = __match.slice(1);
    return (1 + length(xs));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function main() {
  return length([].concat([1], [2], [3], [4], [5]));
}

