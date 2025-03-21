from dataclasses import dataclass
from typing import Any


@dataclass
class Chain:
    value: Any


@dataclass
class Cons:
    value: Any


@dataclass
class Nil: ...


def ldash(x, n):
    return (x << n) - x


def coloncolon(h, t):
    return Chain((h, t))


def infinite_list():
    def list_hash(hash, n):
        hash = ldash(hash, 5) + n
        return lambda: coloncolon(hash, list_hash(hash, n + 1))

    return list_hash(0, 0)


def take(c, n):
    if n <= 0:
        return Nil()
    else:
        match c():
            case Chain(x):
                h = x[0]
                t = x[1]
                return Cons((h, take(t, n - 1)))


def main(n):
    return take(infinite_list(), n)
