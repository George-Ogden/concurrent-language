from dataclasses import dataclass
from typing import Any


@dataclass
class Chain:
    value: Any


def ldash(x, n):
    return (x << n) - x


def coloncolon(h, t):
    return Chain((h, t))


def infinite_list():
    def list_hash(hash, n):
        hash = ldash(hash, 5) + n
        return lambda: coloncolon(hash, list_hash(hash, n + 1))

    return list_hash(0, 0)


def prefix_xor(c, n):
    if n <= 0:
        return 0
    else:
        match c():
            case Chain(x):
                h = x[0]
                t = x[1]
                return h ^ prefix_xor(t, n - 1)


def main(n):
    return prefix_xor(infinite_list(), n)
