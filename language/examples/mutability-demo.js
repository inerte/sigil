export function main() {
  return [].concat([1], [2], [3], [4], [5]).reduce(((a, x) => (a + x)), 0);
}

