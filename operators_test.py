import pytest

from operators import Associativity, OperatorManager

L = Associativity.LEFT
R = Associativity.RIGHT

operators = [
    ("$", L, 0),
    ("|>", R, 1),
    ("::", R, 2),
    ("++", R, 3),
    ("**", R, 4),
    ("*", R, 5),
    ("/", R, 5),
    ("%", R, 5),
    ("+", R, 6),
    ("-", R, 6),
    (">>", R, 7),
    ("<<", R, 7),
    ("<=>", R, 8),
    ("<", R, 9),
    ("<=", R, 9),
    (">", R, 9),
    (">=", R, 9),
    ("==", R, 9),
    ("!=", R, 9),
    ("&", R, 11),
    ("^", R, 12),
    ("|", R, 13),
    ("&&", R, 14),
    ("||", R, 15),
    ("@", L, 16),
]


@pytest.mark.parametrize(
    "operator,associativity",
    ((operator, associativity) for operator, associativity, priority in operators),
)
def test_associativity(operator, associativity):
    assert OperatorManager.get_associativity(operator) == associativity
