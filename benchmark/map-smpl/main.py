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


def gt0(x):
    return x > 0


def lt0(x):
    return x < 0


def main(n):
    xs = finite_list(n)
    return (map(gt0, xs), map(lt0, xs))
