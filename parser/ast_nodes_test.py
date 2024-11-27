import pytest
from ast_nodes import (
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    FunctionType,
    GenericType,
    TupleType,
)


@pytest.mark.parametrize(
    "node,json",
    [
        (AtomicTypeEnum.INT, "INT"),
        (AtomicTypeEnum.BOOL, "BOOL"),
        (AtomicType.BOOL, {"type_": "BOOL"}),
        (TupleType([]), {"types": []}),
        (
            TupleType([AtomicType.INT, TupleType([])]),
            {
                "types": [
                    {"AtomicType": {"type_": "INT"}},
                    {"TupleType": {"types": []}},
                ]
            },
        ),
        (
            FunctionType(TupleType([AtomicType.INT]), AtomicType.INT),
            {
                "argument_type": {"TupleType": {"types": [{"AtomicType": {"type_": "INT"}}]}},
                "return_type": {"AtomicType": {"type_": "INT"}},
            },
        ),
        (
            GenericType("map", [AtomicType.INT, AtomicType.BOOL]),
            {
                "id": "map",
                "type_variables": [
                    {"AtomicType": {"type_": "INT"}},
                    {"AtomicType": {"type_": "BOOL"}},
                ],
            },
        ),
        (
            GenericType(
                "map",
                [FunctionType(AtomicType.INT, AtomicType.INT), GenericType("foo", [])],
            ),
            {
                "id": "map",
                "type_variables": [
                    {
                        "FunctionType": {
                            "argument_type": {"AtomicType": {"type_": "INT"}},
                            "return_type": {"AtomicType": {"type_": "INT"}},
                        }
                    },
                    {"GenericType": {"id": "foo", "type_variables": []}},
                ],
            },
        ),
    ],
)
def test_to_json(node: ASTNode, json: str) -> None:
    assert node.to_json() == json
