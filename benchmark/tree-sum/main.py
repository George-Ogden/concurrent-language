from dataclasses import dataclass
from typing import Any


@dataclass
class Node:
    value: Any


@dataclass
class Leaf: ...


def hash(hash, n):
    return (hash << 5) - hash + n


def gen_tree(n):
    def gen_tree_inner(n, m, h):
        if n <= 0:
            return Leaf()
        else:
            return Node(
                (
                    gen_tree_inner(n - 1, 2 * m, hash(h, m)),
                    h,
                    gen_tree_inner(n - 1, 2 * m + 1, hash(h, m)),
                )
            )

    return gen_tree_inner(n, 0, 0)


def tree_xor(tree):
    match tree:
        case Leaf():
            return 0
        case Node(x):
            l = x[0]
            r = x[2]
            v = x[1]
            return (tree_xor(l) << 1) ^ v ^ (tree_xor(r) << 2)


def main(n):
    tree = gen_tree(n)
    return tree_xor(tree)
