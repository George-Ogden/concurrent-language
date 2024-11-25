from typing import Optional

import pytest

from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    Block,
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
        ("map<T>", GenericVariable("map", [GenericVariable("T", [])]), "expr"),
        ("map<f<int>>", GenericVariable("map", [GenericVariable("f", [AtomicType.INT])]), "expr"),
        (
            "map<f<g<T>>>",
            GenericVariable(
                "map", [GenericVariable("f", [GenericVariable("g", [GenericVariable("T", [])])])]
            ),
            "expr",
        ),
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
        ("a = 3", Assignment(Assignee("a", []), Integer(3)), "assignment"),
        ("__&&__ = 3", Assignment(Assignee("&&", []), Integer(3)), "assignment"),
        ("__>>__ = 3", Assignment(Assignee(">>", []), Integer(3)), "assignment"),
        ("__>__ = 3", Assignment(Assignee(">", []), Integer(3)), "assignment"),
        ("__$__ = 3", Assignment(Assignee("$", []), Integer(3)), "assignment"),
        ("__$ $__ = 3", None, "assignment"),
        ("a == 3", None, "assignment"),
        ("0 = 3", None, "assignment"),
        ("__=__ = 4", None, "assignment"),
        ("__==__ = 4", Assignment(Assignee("==", []), Integer(4)), "assignment"),
        ("a0 = 0", Assignment(Assignee("a0", []), Integer(0)), "assignment"),
        ("_ = 0", Assignment(Assignee("_", []), Integer(0)), "assignment"),
        ("__ = 0", Assignment(Assignee("__", []), Integer(0)), "assignment"),
        ("___ = 0", Assignment(Assignee("___", []), Integer(0)), "assignment"),
        ("____ = 0", Assignment(Assignee("____", []), Integer(0)), "assignment"),
        ("_____ = 0", Assignment(Assignee("_____", []), Integer(0)), "assignment"),
        (
            "a<T> = f<T>",
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [GenericVariable("T", [])])),
            "assignment",
        ),
        (
            "a<T> = f<T>",
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [GenericVariable("T", [])])),
            "assignment",
        ),
        (
            "a<T,> = t<T,>",
            Assignment(Assignee("a", ["T"]), GenericVariable("t", [GenericVariable("T", [])])),
            "assignment",
        ),
        ("a<T,U> = -4", Assignment(Assignee("a", ["T", "U"]), Integer(-4)), "assignment"),
        (
            "a<T,U> = f<U,T>",
            Assignment(
                Assignee("a", ["T", "U"]),
                GenericVariable("f", [GenericVariable("U", []), GenericVariable("T", [])]),
            ),
            "assignment",
        ),
        ("a<T,U,> = 0", Assignment(Assignee("a", ["T", "U"]), Integer(0)), "assignment"),
        ("{5}", Block([], Integer(5)), "block"),
        ("{}", None, "block"),
        ("{a = -9; 8}", Block([Assignment(Assignee("a", []), Integer(-9))], Integer(8)), "block"),
        ("{a = -9}", None, "block"),
        ("{a = -9;}", None, "block"),
        ("{; 8}", None, "block"),
        ("{w = x;; 8}", None, "block"),
        (
            "{w = x;y<T> = x<T,T>; -8}",
            Block(
                [
                    Assignment(Assignee("w", []), GenericVariable("x", [])),
                    Assignment(
                        Assignee("y", ["T"]),
                        GenericVariable("x", [GenericVariable("T", []), GenericVariable("T", [])]),
                    ),
                ],
                Integer(-8),
            ),
            "block",
        ),
        (
            "{w = x; ()}",
            Block([Assignment(Assignee("w", []), GenericVariable("x", []))], TupleExpression([])),
            "block",
        ),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
