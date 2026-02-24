def is_even(x: int) -> bool:
    return x % 2 == 0

def filter_even(xs: list[int]) -> list[int]:
    return list(filter(is_even, xs))

def main() -> list[int]:
    return filter_even([1, 2, 3, 4, 5, 6])
