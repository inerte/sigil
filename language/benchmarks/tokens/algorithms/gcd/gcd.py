def gcd(a: int, b: int) -> int:
    if b == 0:
        return a
    return gcd(b, a % b)

def main() -> int:
    return gcd(48, 18)
