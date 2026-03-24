def double(x: int) -> int:
    return x * 2

def map_double(xs: list[int]) -> list[int]:
    return list(map(double, xs))

def main() -> list[int]:
    return map_double([1, 2, 3, 4, 5])
