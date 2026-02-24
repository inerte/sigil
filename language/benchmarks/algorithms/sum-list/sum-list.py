from functools import reduce

def sum_list(xs: list[int]) -> int:
    return reduce(lambda a, x: a + x, xs, 0)

def main() -> int:
    return sum_list([1, 2, 3, 4, 5])
