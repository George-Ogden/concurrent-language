from typing import Optional

import pytest

from ast_nodes import ASTNode, Integer
from parse import Parser


@pytest.mark.parametrize(
    "code,node,target",
    [
        ("5", Integer(5), "integer"),
        ("0", Integer(0), "integer"),
        ("-8", Integer(-8), "integer"),
        ("10", Integer(10), "integer"),
        ("05", None, "integer"),
        ("-07", None, "integer"),
        ("00", None, "integer"),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
