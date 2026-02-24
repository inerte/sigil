function reverse(s: string): string {
  return s.split('').reverse().join('');
}

function isPalindrome(s: string): boolean {
  return s === reverse(s);
}

function main(): boolean {
  return isPalindrome("racecar");
}
