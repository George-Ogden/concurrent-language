import pytest
from ast_nodes import ASTNode, AtomicTypeEnum


@pytest.mark.parametrize(
    "node,json",
    [
        (AtomicTypeEnum.INT, '"INT"'),
        (AtomicTypeEnum.BOOL, '"BOOL"'),
    ],
)
def test_to_json(node: ASTNode, json: str) -> None:
    assert node.to_json() == json
