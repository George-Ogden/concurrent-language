from dataclasses import dataclass
from typing import Any


@dataclass
class Node:
    value: Any


@dataclass
class Leaf: ...


def gen_tree(n):
    def gen_tree_inner(n, m):
        if n <= 0:
            return Leaf()
        else:
            return Node((gen_tree_inner(n - 1, 2 * m), m, gen_tree_inner(n - 1, 2 * m + 1)))

    return gen_tree_inner(n, 0)


def main(n):
    return gen_tree(n)
