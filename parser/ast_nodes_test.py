import pytest
from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    Block,
    Boolean,
    ElementAccess,
    EmptyTypeDefinition,
    FunctionType,
    GenericType,
    GenericTypeVariable,
    GenericVariable,
    IfExpression,
    Integer,
    MatchItem,
    OpaqueTypeDefinition,
    TransparentTypeDefinition,
    TupleExpression,
    TupleType,
    TypeItem,
    Typename,
    TypeVariable,
    UnionTypeDefinition,
    Variable,
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
                    {
                        "id": "Some",
                        "type_": {"GenericType": {"id": "T", "type_variables": []}},
                    },
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
        (
            TransparentTypeDefinition(
                TypeVariable("ii"),
                TupleType([AtomicType.INT, AtomicType.INT]),
            ),
            {
                "variable": {"id": "ii", "generic_variables": []},
                "type_": {
                    "TupleType": {
                        "types": [
                            {"AtomicType": {"type_": "INT"}},
                            {"AtomicType": {"type_": "INT"}},
                        ]
                    }
                },
            },
        ),
        (Integer(128), {"value": 128}),
        (Integer(-128), {"value": -128}),
        (Boolean(True), {"value": True}),
        (TupleExpression([]), {"expressions": []}),
        (
            TupleExpression([Boolean(False), Integer(5)]),
            {"expressions": [{"Boolean": {"value": False}}, {"Integer": {"value": 5}}]},
        ),
        (
            TupleExpression([TupleExpression([Boolean(False), Integer(5)]), TupleExpression([])]),
            {
                "expressions": [
                    {
                        "TupleExpression": {
                            "expressions": [
                                {"Boolean": {"value": False}},
                                {"Integer": {"value": 5}},
                            ]
                        }
                    },
                    {"TupleExpression": {"expressions": []}},
                ]
            },
        ),
        (Variable("foo"), {"name": "foo", "type_instances": []}),
        (
            GenericVariable("map", [AtomicType.INT]),
            {"name": "map", "type_instances": [{"AtomicType": {"type_": "INT"}}]},
        ),
        (
            GenericVariable("foo", [Typename("T")]),
            {
                "name": "foo",
                "type_instances": [{"GenericType": {"id": "T", "type_variables": []}}],
            },
        ),
        (
            ElementAccess(TupleExpression([Integer(0)]), 0),
            {
                "expression": {"TupleExpression": {"expressions": [{"Integer": {"value": 0}}]}},
                "index": 0,
            },
        ),
        (
            ElementAccess(ElementAccess(Variable("foo"), 13), 1),
            {
                "expression": {
                    "ElementAccess": {
                        "expression": {"GenericVariable": {"name": "foo", "type_instances": []}},
                        "index": 13,
                    }
                },
                "index": 1,
            },
        ),
        (Assignee("a", []), {"id": "a", "generic_variables": []}),
        (Assignee("f", ["T", "U"]), {"id": "f", "generic_variables": ["T", "U"]}),
        (
            Assignment(Assignee("a", []), Variable("b")),
            {
                "assignee": {"id": "a", "generic_variables": []},
                "expression": {"GenericVariable": {"name": "b", "type_instances": []}},
            },
        ),
        (
            Assignment(Assignee("a", ["T"]), GenericVariable("b", [Typename("T")])),
            {
                "assignee": {"id": "a", "generic_variables": ["T"]},
                "expression": {
                    "GenericVariable": {
                        "name": "b",
                        "type_instances": [{"GenericType": {"id": "T", "type_variables": []}}],
                    }
                },
            },
        ),
        (
            Block(
                [
                    Assignment(Assignee("a", []), Variable("x")),
                    Assignment(Assignee("b", []), Integer(3)),
                ],
                Integer(4),
            ),
            {
                "assignments": [
                    {
                        "assignee": {"id": "a", "generic_variables": []},
                        "expression": {"GenericVariable": {"name": "x", "type_instances": []}},
                    },
                    {
                        "assignee": {"id": "b", "generic_variables": []},
                        "expression": {"Integer": {"value": 3}},
                    },
                ],
                "expression": {"Integer": {"value": 4}},
            },
        ),
        (
            IfExpression(Boolean(True), Block([], Integer(1)), Block([], Integer(-1))),
            {
                "condition": {"Boolean": {"value": True}},
                "true_block": {
                    "assignments": [],
                    "expression": {"Integer": {"value": 1}},
                },
                "false_block": {
                    "assignments": [],
                    "expression": {"Integer": {"value": -1}},
                },
            },
        ),
        (
            IfExpression(
                IfExpression(Boolean(True), Block([], Boolean(True)), Block([], Boolean(False))),
                Block(
                    [],
                    IfExpression(Boolean(False), Block([], Integer(1)), Block([], Integer(0))),
                ),
                Block([], Integer(-1)),
            ),
            {
                "condition": {
                    "IfExpression": {
                        "condition": {"Boolean": {"value": True}},
                        "true_block": {
                            "assignments": [],
                            "expression": {"Boolean": {"value": True}},
                        },
                        "false_block": {
                            "assignments": [],
                            "expression": {"Boolean": {"value": False}},
                        },
                    }
                },
                "true_block": {
                    "assignments": [],
                    "expression": {
                        "IfExpression": {
                            "condition": {"Boolean": {"value": False}},
                            "true_block": {
                                "assignments": [],
                                "expression": {"Integer": {"value": 1}},
                            },
                            "false_block": {
                                "assignments": [],
                                "expression": {"Integer": {"value": 0}},
                            },
                        }
                    },
                },
                "false_block": {"assignments": [], "expression": {"Integer": {"value": -1}}},
            },
        ),
        (
            MatchItem("Some", Assignee("x", [])),
            {"type_name": "Some", "assignee": {"id": "x", "generic_variables": []}},
        ),
        (MatchItem("None", None), {"type_name": "None", "assignee": None}),
    ],
)
def test_to_json(node: ASTNode, json: str) -> None:
    assert node.to_json() == json
