from typing import Optional

import pytest

from ast_nodes import (
    ASTNode,
    AtomicType,
    Boolean,
    FunctionType,
    GenericVariable,
    Integer,
    TupleExpression,
    TupleType,
)
from parse import Parser


@pytest.mark.parametrize(
    "code,node,target",
    [
        ("int", AtomicType.INT, "type_instance"),
        ("bool", AtomicType.BOOL, "type_instance"),
        ("(int)", AtomicType.INT, "type_instance"),
        ("((int))", AtomicType.INT, "type_instance"),
        ("foo", GenericVariable("foo", []), "type_instance"),
        ("foo<int>", GenericVariable("foo", [AtomicType.INT]), "type_instance"),
        ("foo<int,>", GenericVariable("foo", [AtomicType.INT]), "type_instance"),
        (
            "foo<int,bool>",
            GenericVariable("foo", [AtomicType.INT, AtomicType.BOOL]),
            "type_instance",
        ),
        (
            "foo<bar<int>,bool>",
            GenericVariable(
                "foo",
                [
                    GenericVariable("bar", [AtomicType.INT]),
                    AtomicType.BOOL,
                ],
            ),
            "type_instance",
        ),
        (
            "(int,bool)",
            TupleType([AtomicType.INT, AtomicType.BOOL]),
            "type_instance",
        ),
        (
            "(int,bool)",
            TupleType([AtomicType.INT, AtomicType.BOOL]),
            "type_instance",
        ),
        (
            "(int,)",
            TupleType([AtomicType.INT]),
            "type_instance",
        ),
        (
            "()",
            TupleType([]),
            "type_instance",
        ),
        (
            "((int,int),(bool,bool))",
            TupleType(
                [
                    TupleType([AtomicType.INT, AtomicType.INT]),
                    TupleType([AtomicType.BOOL, AtomicType.BOOL]),
                ]
            ),
            "type_instance",
        ),
        (
            "((),)",
            TupleType([TupleType([])]),
            "type_instance",
        ),
        (
            "(int,bool)->int",
            FunctionType(TupleType([AtomicType.INT, AtomicType.BOOL]), AtomicType.INT),
            "type_instance",
        ),
        (
            "(int,bool,)->int",
            FunctionType(TupleType([AtomicType.INT, AtomicType.BOOL]), AtomicType.INT),
            "type_instance",
        ),
        ("(int,)->int", FunctionType(TupleType([AtomicType.INT]), AtomicType.INT), "type_instance"),
        ("(int)->int", FunctionType(AtomicType.INT, AtomicType.INT), "type_instance"),
        ("int->int", FunctionType(AtomicType.INT, AtomicType.INT), "type_instance"),
        ("()->()", FunctionType(TupleType([]), TupleType([])), "type_instance"),
        (
            "int->bool->()",
            FunctionType(AtomicType.INT, FunctionType(AtomicType.BOOL, TupleType([]))),
            "type_instance",
        ),
        (
            "(int->bool->())",
            FunctionType(AtomicType.INT, FunctionType(AtomicType.BOOL, TupleType([]))),
            "type_instance",
        ),
        (
            "int->(bool->())",
            FunctionType(AtomicType.INT, FunctionType(AtomicType.BOOL, TupleType([]))),
            "type_instance",
        ),
        (
            "(int)->(bool->())",
            FunctionType(AtomicType.INT, FunctionType(AtomicType.BOOL, TupleType([]))),
            "type_instance",
        ),
        (
            "(int->bool)->()",
            FunctionType(FunctionType(AtomicType.INT, AtomicType.BOOL), TupleType([])),
            "type_instance",
        ),
        (
            "(int->(int,),)->(())",
            FunctionType(
                TupleType([FunctionType(AtomicType.INT, TupleType([AtomicType.INT]))]),
                TupleType([]),
            ),
            "type_instance",
        ),
        ("5", Integer(5), "expr"),
        ("0", Integer(0), "expr"),
        ("-8", Integer(-8), "expr"),
        ("10", Integer(10), "expr"),
        ("05", None, "expr"),
        ("-07", None, "expr"),
        ("00", None, "expr"),
        ("true", Boolean(True), "expr"),
        ("false", Boolean(False), "expr"),
        ("x", GenericVariable("x", []), "expr"),
        ("foo", GenericVariable("foo", []), "expr"),
        ("r2d2", GenericVariable("r2d2", []), "expr"),
        ("map<int>", GenericVariable("map", [AtomicType.INT]), "expr"),
        ("map<int,>", GenericVariable("map", [AtomicType.INT]), "expr"),
        (
            "map<int,bool>",
            GenericVariable("map", [AtomicType.INT, AtomicType.BOOL]),
            "expr",
        ),
        (
            "map<(int,int)>",
            GenericVariable("map", [TupleType([AtomicType.INT, AtomicType.INT])]),
            "expr",
        ),
        (
            "()",
            TupleExpression([]),
            "expr",
        ),
        (
            "(3,)",
            TupleExpression([Integer(3)]),
            "expr",
        ),
        (
            "(8,5,)",
            TupleExpression([Integer(8), Integer(5)]),
            "expr",
        ),
        (
            "(8,5)",
            TupleExpression([Integer(8), Integer(5)]),
            "expr",
        ),
        (
            "(())",
            TupleExpression([]),
            "expr",
        ),
        (
            "((),)",
            TupleExpression([TupleExpression([])]),
            "expr",
        ),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
