from dataclasses import dataclass
from typing import Any


@dataclass
class Cons:
    value: Any


@dataclass
class Nil: ...


def coloncolon(h, t):
    return Cons((h, t))


def map(f, xs):
    def coloncolon(h, t):
        return Cons((h, t))

    match xs:
        case Cons(x):
            h = x[0]
            t = x[1]
            return coloncolon(f(h), map(f, t))
        case Nil():
            return Nil()


def ldash(x, n):
    return (x << n) - x


def finite_list(n):
    def list_hash(hash, n):
        if n == 0:
            return Nil()
        else:
            hash = ldash(hash, 5) + n
            return coloncolon(hash, list_hash(hash, n - 1))

    return list_hash(0, n)


def int_hash(m, x):
    if m == 0:
        return 0
    else:
        return ldash(int_hash(m - 1, x), 5) + x


def main(m, n):
    xs = finite_list(n)
    return map(lambda x: int_hash(m, x), xs)
