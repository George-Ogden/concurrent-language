import pytest

from ast_nodes import Assignee, Assignment, FunctionCall, Variable
from operators import Associativity, OperatorManager
from parse import Parser

L = Associativity.LEFT
R = Associativity.RIGHT
N = Associativity.NONE

operators = [
    ("$", L, 0),
    ("|>", R, 1),
    ("::", L, 2),
    ("++", L, 3),
    ("**", L, 4),
    ("*", R, 5),
    ("/", R, 5),
    ("%", R, 5),
    ("+", R, 6),
    ("-", R, 6),
    (">>", R, 7),
    ("<<", R, 7),
    ("<=>", N, 8),
    ("<", N, 9),
    ("<=", N, 9),
    (">", N, 9),
    (">=", N, 9),
    ("==", N, 9),
    ("!=", N, 9),
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


@pytest.mark.parametrize(
    "operator",
    (operator for operator, associativity, priority in operators),
)
def test_parsing(operator):
    code = f"x {operator} y"
    ast = Parser.parse(code, target="expr")
    node = FunctionCall(Variable(operator), [Variable("x"), Variable("y")])
    assert ast == node

    code = f"__{operator}__ = x"
    ast = Parser.parse(code, target="assignment")
    node = Assignment(Assignee(operator, []), Variable("x"))
    assert ast == node
