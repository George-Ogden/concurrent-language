import pytest
from ast_nodes import (
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    EmptyTypeDefinition,
    FunctionType,
    GenericType,
    GenericTypeVariable,
    OpaqueTypeDefinition,
    TupleType,
    TypeItem,
    Typename,
    UnionTypeDefinition,
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
        (
            UnionTypeDefinition(
                GenericTypeVariable("Maybe", ["T"]),
                [TypeItem("Some", Typename("T")), TypeItem("None", None)],
            ),
            {
                "variable": {"id": "Maybe", "generic_variables": ["T"]},
                "items": [
                    {"id": "Some", "type_": {"GenericType": {"id": "T", "type_variables": []}}},
                    {"id": "None", "type_": None},
                ],
            },
        ),
        (
            OpaqueTypeDefinition(
                GenericTypeVariable("Pair", ["T", "U"]),
                TupleType([Typename("T"), Typename("U")]),
            ),
            {
                "variable": {"id": "Pair", "generic_variables": ["T", "U"]},
                "type_": {
                    "TupleType": {
                        "types": [
                            {"GenericType": {"id": "T", "type_variables": []}},
                            {"GenericType": {"id": "U", "type_variables": []}},
                        ]
                    }
                },
            },
        ),
        (
            EmptyTypeDefinition("None"),
            {"id": "None"},
        ),
    ],
)
def test_to_json(node: ASTNode, json: str) -> None:
    assert node.to_json() == json
