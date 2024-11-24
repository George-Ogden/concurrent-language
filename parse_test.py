from typing import Optional

import pytest

from ast_nodes import ASTNode, GenericVariable, Integer
from parse import Parser


@pytest.mark.parametrize(
    "code,node,target",
    [
        ("5", Integer(5), "expr"),
        ("0", Integer(0), "expr"),
        ("-8", Integer(-8), "expr"),
        ("10", Integer(10), "expr"),
        ("05", None, "expr"),
        ("-07", None, "expr"),
        ("00", None, "expr"),
        ("x", GenericVariable("x", []), "expr"),
        ("foo", GenericVariable("foo", []), "expr"),
        ("r2d2", GenericVariable("r2d2", []), "expr"),
        ("map<int>", GenericVariable("map", ["int"]), "expr"),
        ("map<int,>", GenericVariable("map", ["int"]), "expr"),
        ("map<int,bool>", GenericVariable("map", ["int", "bool"]), "expr"),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
