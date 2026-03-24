def reverse(s: str) -> str:
    return s[::-1]

def is_palindrome(s: str) -> bool:
    return s == reverse(s)

def main() -> bool:
    return is_palindrome("racecar")
