from parser import Parser

import pytest
from ast_nodes import Assignee, Assignment, FunctionCall, ParametricAssignee, Variable
from operators import Associativity, OperatorManager

L = Associativity.LEFT
R = Associativity.RIGHT
N = Associativity.NONE

operators = [
    (".", R, 1),
    ("@", L, 2),
    ("**", L, 3),
    ("*", R, 4),
    ("/", R, 5),
    ("%", R, 6),
    ("+", R, 7),
    ("-", R, 7),
    (">>", R, 8),
    ("<<", R, 8),
    ("::", L, 9),
    ("++", L, 9),
    ("<=>", N, 10),
    ("<", N, 11),
    ("<=", N, 11),
    (">", N, 11),
    (">=", N, 11),
    ("==", N, 11),
    ("!=", N, 11),
    ("&", R, 12),
    ("^", R, 13),
    ("|", R, 14),
    ("&&", R, 15),
    ("||", R, 16),
    ("|>", R, 17),
    ("$", L, 18),
]


@pytest.mark.parametrize(
    "operator,associativity",
    ((operator, associativity) for operator, associativity, precedence in operators),
)
def test_associativity(operator, associativity):
    assert OperatorManager.get_associativity(operator) == associativity


@pytest.mark.parametrize(
    "operator",
    (operator for operator, associativity, precedence in operators if operator != "."),
)
def test_parsing(operator):
    code = f"x {operator} y"
    ast = Parser.parse(code, target="expr")
    node = FunctionCall(Variable(operator), [Variable("x"), Variable("y")])
    assert ast == node

    code = f"__{operator}__ = x"
    ast = Parser.parse(code, target="assignment")
    node = Assignment(ParametricAssignee(Assignee(operator), []), Variable("x"))
    assert ast == node


def test_parsing_valid_operator():
    operator = ".."
    code = f"x {operator} y"
    ast = Parser.parse(code, target="expr")
    node = FunctionCall(Variable(".."), [Variable("x"), Variable("y")])
    assert ast == node

    code = f"x{operator}y"
    ast = Parser.parse(code, target="expr")
    assert ast == node


def test_parsing_invalid_operator():
    operator = "??"
    code = f"x {operator} y"
    ast = Parser.parse(code, target="expr")
    assert ast == None

    code = f"__{operator}__ = x"
    ast = Parser.parse(code, target="assignment")
    assert ast == None


@pytest.mark.parametrize(
    "operator,precedence",
    ((operator, precedence) for operator, associativity, precedence in operators),
)
def test_precedence(operator, precedence):
    operator1, precedence1 = operator, precedence
    for operator2, _, precedence2 in operators:
        if operator1 == operator2:
            assert OperatorManager.get_precedence(operator1) == OperatorManager.get_precedence(
                operator2
            )
        elif precedence1 < precedence2:
            assert OperatorManager.get_precedence(operator1) < OperatorManager.get_precedence(
                operator2
            )
        elif precedence1 > precedence2:
            assert OperatorManager.get_precedence(operator1) > OperatorManager.get_precedence(
                operator2
            )
