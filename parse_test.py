from typing import Optional

import pytest

from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    Block,
    Boolean,
    FunctionCall,
    FunctionType,
    GenericVariable,
    Id,
    IfExpression,
    Integer,
    MatchBlock,
    MatchExpression,
    MatchItem,
    TupleExpression,
    TupleType,
)
from parse import Parser


def Variable(name: Id) -> GenericVariable:
    return GenericVariable(name, [])


@pytest.mark.parametrize(
    "code,node,target",
    [
        ("int", AtomicType.INT, "type_instance"),
        ("bool", AtomicType.BOOL, "type_instance"),
        ("(int)", AtomicType.INT, "type_instance"),
        ("((int))", AtomicType.INT, "type_instance"),
        ("foo", Variable("foo"), "type_instance"),
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
        ("x", Variable("x"), "expr"),
        ("foo", Variable("foo"), "expr"),
        ("r2d2", Variable("r2d2"), "expr"),
        ("map<int>", GenericVariable("map", [AtomicType.INT]), "expr"),
        ("map<int,>", GenericVariable("map", [AtomicType.INT]), "expr"),
        ("map<T>", GenericVariable("map", [Variable("T")]), "expr"),
        ("map<f<int>>", GenericVariable("map", [GenericVariable("f", [AtomicType.INT])]), "expr"),
        (
            "map<f<g<T>>>",
            GenericVariable("map", [GenericVariable("f", [GenericVariable("g", [Variable("T")])])]),
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
        ("3 + 4", FunctionCall("+", [Integer(3), Integer(4)]), "expr"),
        ("3 * 4", FunctionCall("*", [Integer(3), Integer(4)]), "expr"),
        ("3 &&$& 4", FunctionCall("&&$&", [Integer(3), Integer(4)]), "expr"),
        ("3 __add__ 4", FunctionCall("add", [Integer(3), Integer(4)]), "expr"),
        ("3 ____ 4", None, "expr"),
        ("3 __^__ 4", None, "expr"),
        ("3 _____ 4", FunctionCall("_", [Integer(3), Integer(4)]), "expr"),
        ("3 ______ 4", FunctionCall("__", [Integer(3), Integer(4)]), "expr"),
        (
            "3 + 4 + 5",
            FunctionCall("+", [FunctionCall("+", [Integer(3), Integer(4)]), Integer(5)]),
            "expr",
        ),
        (
            "3 * 4 + 5",
            FunctionCall("+", [FunctionCall("*", [Integer(3), Integer(4)]), Integer(5)]),
            "expr",
        ),
        (
            "3 + 4 + 5 + 6",
            FunctionCall(
                "+",
                [
                    FunctionCall("+", [FunctionCall("+", [Integer(3), Integer(4)]), Integer(5)]),
                    Integer(6),
                ],
            ),
            "expr",
        ),
        (
            "3 __add__ 4 __add__ 5 __add__ 6",
            FunctionCall(
                "add",
                [
                    FunctionCall(
                        "add", [FunctionCall("add", [Integer(3), Integer(4)]), Integer(5)]
                    ),
                    Integer(6),
                ],
            ),
            "expr",
        ),
        (
            "3 + 4 * 5",
            FunctionCall("+", [Integer(3), FunctionCall("*", [Integer(4), Integer(5)])]),
            "expr",
        ),
        (
            "(3 + 4) * 5",
            FunctionCall("*", [FunctionCall("+", [Integer(3), Integer(4)]), Integer(5)]),
            "expr",
        ),
        (
            "2 * 3 + 4 * 5",
            FunctionCall(
                "+",
                [
                    FunctionCall("*", [Integer(2), Integer(3)]),
                    FunctionCall("*", [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "foo()",
            FunctionCall(Variable("foo"), []),
            "expr",
        ),
        (
            "foo(4,)",
            FunctionCall(Variable("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4)",
            FunctionCall(Variable("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4,5)",
            FunctionCall(Variable("foo"), [Integer(4), Integer(5)]),
            "expr",
        ),
        (
            "foo(4,5,)",
            FunctionCall(Variable("foo"), [Integer(4), Integer(5)]),
            "expr",
        ),
        (
            "(foo)(4)",
            FunctionCall(Variable("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4)(-5,0)",
            FunctionCall(FunctionCall(Variable("foo"), [Integer(4)]), [Integer(-5), Integer(0)]),
            "expr",
        ),
        (
            "foo(4)(a)(-5,bar(true))",
            FunctionCall(
                FunctionCall(FunctionCall(Variable("foo"), [Integer(4)]), [Variable("a")]),
                [Integer(-5), FunctionCall(Variable("bar"), [Boolean(True)])],
            ),
            "expr",
        ),
        ("a = 3", Assignment(Assignee("a", []), Integer(3)), "assignment"),
        ("__a__ = 3", Assignment(Assignee("__a__", []), Integer(3)), "assignment"),
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
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [Variable("T")])),
            "assignment",
        ),
        (
            "a<T> = f<T>",
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [Variable("T")])),
            "assignment",
        ),
        (
            "a<T,> = t<T,>",
            Assignment(Assignee("a", ["T"]), GenericVariable("t", [Variable("T")])),
            "assignment",
        ),
        ("a<T,U> = -4", Assignment(Assignee("a", ["T", "U"]), Integer(-4)), "assignment"),
        (
            "a<T,U> = f<U,T>",
            Assignment(
                Assignee("a", ["T", "U"]),
                GenericVariable("f", [Variable("U"), Variable("T")]),
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
                    Assignment(Assignee("w", []), Variable("x")),
                    Assignment(
                        Assignee("y", ["T"]),
                        GenericVariable("x", [Variable("T"), Variable("T")]),
                    ),
                ],
                Integer(-8),
            ),
            "block",
        ),
        (
            "{w = x; ()}",
            Block([Assignment(Assignee("w", []), Variable("x"))], TupleExpression([])),
            "block",
        ),
        (
            "if (g) { 1 } else { 2 }",
            IfExpression(Variable("g"), Block([], Integer(1)), Block([], Integer(2))),
            "expr",
        ),
        (
            "if (x > 0) { x = 0; true } else { x = 1; false }",
            IfExpression(
                FunctionCall(">", [Variable("x"), Integer(0)]),
                Block([Assignment(Assignee("x", []), Integer(0))], Boolean(True)),
                Block([Assignment(Assignee("x", []), Integer(1))], Boolean(False)),
            ),
            "expr",
        ),
        (
            "match (maybe()) { Some x: { t }; None : { y };}",
            MatchExpression(
                FunctionCall(Variable("maybe"), []),
                [
                    MatchBlock([MatchItem("Some", Assignee("x", []))], Block([], Variable("t"))),
                    MatchBlock([MatchItem("None", None)], Block([], Variable("y"))),
                ],
            ),
            "expr",
        ),
        (
            "match (maybe()) { Some x: { t }; None : { y }}",
            MatchExpression(
                FunctionCall(Variable("maybe"), []),
                [
                    MatchBlock([MatchItem("Some", Assignee("x", []))], Block([], Variable("t"))),
                    MatchBlock([MatchItem("None", None)], Block([], Variable("y"))),
                ],
            ),
            "expr",
        ),
        (
            "match(()) { Some x | None: { () }; }",
            MatchExpression(
                TupleExpression([]),
                [
                    MatchBlock(
                        [MatchItem("Some", Assignee("x", [])), MatchItem("None", None)],
                        Block([], TupleExpression([])),
                    ),
                ],
            ),
            "expr",
        ),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
