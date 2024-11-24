from typing import Optional

import pytest

from ast_nodes import ASTNode
from parse import Parser


@pytest.mark.parametrize("code,node,target", [("5", 5, "integer")])
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
