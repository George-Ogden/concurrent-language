import pytest
from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    Block,
    Boolean,
    ConstructorCall,
    ElementAccess,
    EmptyTypeDefinition,
    FunctionCall,
    FunctionDefinition,
    FunctionType,
    GenericConstructor,
    GenericType,
    GenericTypeVariable,
    GenericVariable,
    IfExpression,
    Integer,
    MatchBlock,
    MatchExpression,
    MatchItem,
    OpaqueTypeDefinition,
    ParametricAssignee,
    Program,
    TransparentTypeDefinition,
    TupleExpression,
    TupleType,
    TypedAssignee,
    TypeItem,
    Typename,
    TypeVariable,
    UnionTypeDefinition,
    Var,
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
            FunctionType([AtomicType.INT], AtomicType.INT),
            {
                "argument_types": [{"AtomicType": {"type_": "INT"}}],
                "return_type": {"AtomicType": {"type_": "INT"}},
            },
        ),
        (
            FunctionType([TupleType([AtomicType.INT])], TupleType([])),
            {
                "argument_types": [{"TupleType": {"types": [{"AtomicType": {"type_": "INT"}}]}}],
                "return_type": {"TupleType": {"types": []}},
            },
        ),
        (
            FunctionType([FunctionType([AtomicType.INT], AtomicType.INT)], AtomicType.INT),
            {
                "argument_types": [
                    {
                        "FunctionType": {
                            "argument_types": [{"AtomicType": {"type_": "INT"}}],
                            "return_type": {"AtomicType": {"type_": "INT"}},
                        }
                    }
                ],
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
                [
                    FunctionType([AtomicType.INT], AtomicType.INT),
                    GenericType("foo", []),
                ],
            ),
            {
                "id": "map",
                "type_variables": [
                    {
                        "FunctionType": {
                            "argument_types": [{"AtomicType": {"type_": "INT"}}],
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
            OpaqueTypeDefinition(
                GenericTypeVariable("F", []),
                FunctionType([FunctionType([AtomicType.INT], AtomicType.INT)], AtomicType.INT),
            ),
            {
                "variable": {"id": "F", "generic_variables": []},
                "type_": {
                    "FunctionType": {
                        "argument_types": [
                            {
                                "FunctionType": {
                                    "argument_types": [{"AtomicType": {"type_": "INT"}}],
                                    "return_type": {"AtomicType": {"type_": "INT"}},
                                }
                            }
                        ],
                        "return_type": {"AtomicType": {"type_": "INT"}},
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
        (Var("foo"), {"id": "foo", "type_instances": []}),
        (
            GenericVariable("map", [AtomicType.INT]),
            {"id": "map", "type_instances": [{"AtomicType": {"type_": "INT"}}]},
        ),
        (
            GenericVariable("foo", [Typename("T")]),
            {
                "id": "foo",
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
            ElementAccess(ElementAccess(Var("foo"), 13), 1),
            {
                "expression": {
                    "ElementAccess": {
                        "expression": {"GenericVariable": {"id": "foo", "type_instances": []}},
                        "index": 13,
                    }
                },
                "index": 1,
            },
        ),
        (
            ParametricAssignee(Assignee("a"), []),
            {"assignee": {"id": "a"}, "generic_variables": []},
        ),
        (
            ParametricAssignee(Assignee("f"), ["T", "U"]),
            {"assignee": {"id": "f"}, "generic_variables": ["T", "U"]},
        ),
        (
            Assignment(ParametricAssignee(Assignee("a"), []), Var("b")),
            {
                "assignee": {"assignee": {"id": "a"}, "generic_variables": []},
                "expression": {"GenericVariable": {"id": "b", "type_instances": []}},
            },
        ),
        (
            Assignment(
                ParametricAssignee(Assignee("a"), ["T"]),
                GenericVariable("b", [Typename("T")]),
            ),
            {
                "assignee": {"assignee": {"id": "a"}, "generic_variables": ["T"]},
                "expression": {
                    "GenericVariable": {
                        "id": "b",
                        "type_instances": [{"GenericType": {"id": "T", "type_variables": []}}],
                    }
                },
            },
        ),
        (
            Block(
                [
                    Assignment(ParametricAssignee(Assignee("a"), []), Var("x")),
                    Assignment(ParametricAssignee(Assignee("b"), []), Integer(3)),
                ],
                Integer(4),
            ),
            {
                "assignments": [
                    {
                        "assignee": {"assignee": {"id": "a"}, "generic_variables": []},
                        "expression": {"GenericVariable": {"id": "x", "type_instances": []}},
                    },
                    {
                        "assignee": {"assignee": {"id": "b"}, "generic_variables": []},
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
                "false_block": {
                    "assignments": [],
                    "expression": {"Integer": {"value": -1}},
                },
            },
        ),
        (
            MatchItem("Some", Assignee("x")),
            {"type_name": "Some", "assignee": {"id": "x"}},
        ),
        (MatchItem("None", None), {"type_name": "None", "assignee": None}),
        (
            MatchBlock(
                [MatchItem("None", None), MatchItem("Some", Assignee("x"))],
                Block([], Boolean(True)),
            ),
            {
                "matches": [
                    {"type_name": "None", "assignee": None},
                    {"type_name": "Some", "assignee": {"id": "x"}},
                ],
                "block": {
                    "assignments": [],
                    "expression": {"Boolean": {"value": True}},
                },
            },
        ),
        (
            MatchExpression(
                Var("maybe"),
                [
                    MatchBlock([MatchItem("Some", Assignee("x"))], Block([], Boolean(True))),
                    MatchBlock([MatchItem("None", None)], Block([], Boolean(False))),
                ],
            ),
            {
                "subject": {"GenericVariable": {"id": "maybe", "type_instances": []}},
                "blocks": [
                    {
                        "matches": [{"type_name": "Some", "assignee": {"id": "x"}}],
                        "block": {
                            "assignments": [],
                            "expression": {"Boolean": {"value": True}},
                        },
                    },
                    {
                        "matches": [{"type_name": "None", "assignee": None}],
                        "block": {
                            "assignments": [],
                            "expression": {"Boolean": {"value": False}},
                        },
                    },
                ],
            },
        ),
        (
            MatchExpression(
                Var("maybe"),
                [
                    MatchBlock(
                        [MatchItem("Some", Assignee("x"))],
                        Block(
                            [],
                            MatchExpression(
                                Var("x"),
                                [
                                    MatchBlock(
                                        [
                                            MatchItem("Positive", None),
                                        ],
                                        Block([], Integer(1)),
                                    ),
                                    MatchBlock(
                                        [
                                            MatchItem("Negative", None),
                                        ],
                                        Block([], Integer(-1)),
                                    ),
                                ],
                            ),
                        ),
                    ),
                    MatchBlock([MatchItem("None", None)], Block([], Integer(0))),
                ],
            ),
            {
                "subject": {"GenericVariable": {"id": "maybe", "type_instances": []}},
                "blocks": [
                    {
                        "matches": [{"type_name": "Some", "assignee": {"id": "x"}}],
                        "block": {
                            "assignments": [],
                            "expression": {
                                "MatchExpression": {
                                    "subject": {
                                        "GenericVariable": {
                                            "id": "x",
                                            "type_instances": [],
                                        }
                                    },
                                    "blocks": [
                                        {
                                            "matches": [
                                                {
                                                    "type_name": "Positive",
                                                    "assignee": None,
                                                }
                                            ],
                                            "block": {
                                                "assignments": [],
                                                "expression": {"Integer": {"value": 1}},
                                            },
                                        },
                                        {
                                            "matches": [
                                                {
                                                    "type_name": "Negative",
                                                    "assignee": None,
                                                }
                                            ],
                                            "block": {
                                                "assignments": [],
                                                "expression": {"Integer": {"value": -1}},
                                            },
                                        },
                                    ],
                                }
                            },
                        },
                    },
                    {
                        "matches": [{"type_name": "None", "assignee": None}],
                        "block": {
                            "assignments": [],
                            "expression": {"Integer": {"value": 0}},
                        },
                    },
                ],
            },
        ),
        (
            FunctionCall(Var("foo"), [Integer(3), Var("x")]),
            {
                "function": {"GenericVariable": {"id": "foo", "type_instances": []}},
                "arguments": [
                    {"Integer": {"value": 3}},
                    {"GenericVariable": {"id": "x", "type_instances": []}},
                ],
            },
        ),
        (
            ConstructorCall(GenericConstructor("foo", [AtomicType.INT]), [Integer(3), Var("x")]),
            {
                "constructor": {
                    "id": "foo",
                    "type_instances": [{"AtomicType": {"type_": "INT"}}],
                },
                "arguments": [
                    {"Integer": {"value": 3}},
                    {"GenericVariable": {"id": "x", "type_instances": []}},
                ],
            },
        ),
        (
            FunctionDefinition(
                [
                    TypedAssignee(Assignee("x"), AtomicType.INT),
                    TypedAssignee(Assignee("y"), AtomicType.BOOL),
                ],
                AtomicType.BOOL,
                Block([], Var("y")),
            ),
            {
                "parameters": [
                    {
                        "assignee": {"id": "x"},
                        "type_": {"AtomicType": {"type_": "INT"}},
                    },
                    {
                        "assignee": {"id": "y"},
                        "type_": {"AtomicType": {"type_": "BOOL"}},
                    },
                ],
                "return_type": {"AtomicType": {"type_": "BOOL"}},
                "body": {
                    "assignments": [],
                    "expression": {"GenericVariable": {"id": "y", "type_instances": []}},
                },
            },
        ),
        (Program([]), {"definitions": []}),
        (
            Program(
                [
                    OpaqueTypeDefinition(
                        GenericTypeVariable("F", []),
                        FunctionType(
                            [FunctionType([AtomicType.INT], AtomicType.INT)],
                            AtomicType.INT,
                        ),
                    )
                ]
            ),
            {
                "definitions": [
                    {
                        "OpaqueTypeDefinition": {
                            "variable": {"id": "F", "generic_variables": []},
                            "type_": {
                                "FunctionType": {
                                    "argument_types": [
                                        {
                                            "FunctionType": {
                                                "argument_types": [
                                                    {"AtomicType": {"type_": "INT"}}
                                                ],
                                                "return_type": {"AtomicType": {"type_": "INT"}},
                                            }
                                        }
                                    ],
                                    "return_type": {"AtomicType": {"type_": "INT"}},
                                }
                            },
                        }
                    }
                ]
            },
        ),
        (
            Program(
                [
                    UnionTypeDefinition(
                        GenericTypeVariable("Maybe", ["T"]),
                        [TypeItem("Some", Typename("T")), TypeItem("None", None)],
                    ),
                    TransparentTypeDefinition(
                        GenericTypeVariable("Pair", ["T", "U"]),
                        TupleType([Typename("T"), Typename("U")]),
                    ),
                    Assignment(ParametricAssignee(Assignee("a"), []), Var("x")),
                    Assignment(ParametricAssignee(Assignee("b"), []), Integer(3)),
                ]
            ),
            {
                "definitions": [
                    {
                        "UnionTypeDefinition": {
                            "variable": {"id": "Maybe", "generic_variables": ["T"]},
                            "items": [
                                {
                                    "id": "Some",
                                    "type_": {"GenericType": {"id": "T", "type_variables": []}},
                                },
                                {"id": "None", "type_": None},
                            ],
                        }
                    },
                    {
                        "TransparentTypeDefinition": {
                            "variable": {"id": "Pair", "generic_variables": ["T", "U"]},
                            "type_": {
                                "TupleType": {
                                    "types": [
                                        {
                                            "GenericType": {
                                                "id": "T",
                                                "type_variables": [],
                                            }
                                        },
                                        {
                                            "GenericType": {
                                                "id": "U",
                                                "type_variables": [],
                                            }
                                        },
                                    ]
                                }
                            },
                        }
                    },
                    {
                        "Assignment": {
                            "assignee": {
                                "assignee": {"id": "a"},
                                "generic_variables": [],
                            },
                            "expression": {"GenericVariable": {"id": "x", "type_instances": []}},
                        }
                    },
                    {
                        "Assignment": {
                            "assignee": {
                                "assignee": {"id": "b"},
                                "generic_variables": [],
                            },
                            "expression": {"Integer": {"value": 3}},
                        }
                    },
                ]
            },
        ),
    ],
)
def test_to_json(node: ASTNode, json: str) -> None:
    print(node)
    assert node.to_json() == json
