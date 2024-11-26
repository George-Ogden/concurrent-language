from typing import Optional

import pytest

from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    Block,
    Boolean,
    ElementAccess,
    EmptyTypeDefinition,
    FunctionCall,
    FunctionDef,
    FunctionType,
    GenericType,
    GenericTypeVariable,
    GenericVariable,
    IfExpression,
    Integer,
    MatchBlock,
    MatchExpression,
    MatchItem,
    OpaqueTypeDefinition,
    TupleExpression,
    TupleType,
    TypedAssignee,
    Typename,
    TypeVariable,
    Variable,
)
from parse import Parser


@pytest.mark.parametrize(
    "code,node,target",
    [
        ("int", AtomicType.INT, "type_instance"),
        ("bool", AtomicType.BOOL, "type_instance"),
        ("(int)", AtomicType.INT, "type_instance"),
        ("((int))", AtomicType.INT, "type_instance"),
        ("foo", Typename("foo"), "type_instance"),
        ("foo<int>", GenericType("foo", [AtomicType.INT]), "type_instance"),
        ("foo<int,>", GenericType("foo", [AtomicType.INT]), "type_instance"),
        (
            "foo<int,bool>",
            GenericType("foo", [AtomicType.INT, AtomicType.BOOL]),
            "type_instance",
        ),
        (
            "foo<bar<int>,bool>",
            GenericType(
                "foo",
                [
                    GenericType("bar", [AtomicType.INT]),
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
        (
            "(int,)->int",
            FunctionType(TupleType([AtomicType.INT]), AtomicType.INT),
            "type_instance",
        ),
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
        ("map<T>", GenericVariable("map", [Typename("T")]), "expr"),
        (
            "map<f<int>>",
            GenericVariable("map", [GenericType("f", [AtomicType.INT])]),
            "expr",
        ),
        (
            "map<f<g<T>>>",
            GenericVariable("map", [GenericType("f", [GenericType("g", [Typename("T")])])]),
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
        ("3 + 4", FunctionCall(Variable("+"), [Integer(3), Integer(4)]), "expr"),
        ("3 * 4", FunctionCall(Variable("*"), [Integer(3), Integer(4)]), "expr"),
        ("3 &&$& 4", FunctionCall(Variable("&&$&"), [Integer(3), Integer(4)]), "expr"),
        (
            "3 __add__ 4",
            FunctionCall(Variable("add"), [Integer(3), Integer(4)]),
            "expr",
        ),
        ("3 ____ 4", None, "expr"),
        ("3 __^__ 4", None, "expr"),
        ("3 _____ 4", FunctionCall(Variable("_"), [Integer(3), Integer(4)]), "expr"),
        ("3 ______ 4", FunctionCall(Variable("__"), [Integer(3), Integer(4)]), "expr"),
        (
            "3 + 4 + 5",
            FunctionCall(
                Variable("+"),
                [FunctionCall(Variable("+"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "3 * 4 + 5",
            FunctionCall(
                Variable("+"),
                [FunctionCall(Variable("*"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "3 + 4 + 5 + 6",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            FunctionCall(Variable("+"), [Integer(3), Integer(4)]),
                            Integer(5),
                        ],
                    ),
                    Integer(6),
                ],
            ),
            "expr",
        ),
        (
            "3 __add__ 4 __add__ 5 __add__ 6",
            FunctionCall(
                Variable("add"),
                [
                    Integer(3),
                    FunctionCall(
                        Variable("add"),
                        [
                            Integer(4),
                            FunctionCall(Variable("add"), [Integer(5), Integer(6)]),
                        ],
                    ),
                ],
            ),
            "expr",
        ),
        (
            "3 + 4 * 5",
            FunctionCall(
                Variable("+"),
                [Integer(3), FunctionCall(Variable("*"), [Integer(4), Integer(5)])],
            ),
            "expr",
        ),
        (
            "(3 + 4) * 5",
            FunctionCall(
                Variable("*"),
                [FunctionCall(Variable("+"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "2 * 3 + 4 * 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(Variable("*"), [Integer(2), Integer(3)]),
                    FunctionCall(Variable("*"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "3 __mul__ 4 + 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(Variable("mul"), [Integer(3), Integer(4)]),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 * 3 + 4 + 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            FunctionCall(Variable("*"), [Integer(2), Integer(3)]),
                            Integer(4),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 + 4 * 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            Integer(2),
                            Integer(3),
                        ],
                    ),
                    FunctionCall(Variable("*"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 * 4 + 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            Integer(2),
                            FunctionCall(Variable("*"), [Integer(3), Integer(4)]),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 __mul__ 4 + 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            Integer(2),
                            FunctionCall(Variable("mul"), [Integer(3), Integer(4)]),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 <<!>> 4 + 5",
            FunctionCall(
                Variable("+"),
                [
                    FunctionCall(
                        Variable("+"),
                        [
                            Integer(2),
                            FunctionCall(Variable("<<!>>"), [Integer(3), Integer(4)]),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 __add__ 3 <<!>> 4 __add__ 5",
            FunctionCall(
                Variable("<<!>>"),
                [
                    FunctionCall(Variable("add"), [Integer(2), Integer(3)]),
                    FunctionCall(Variable("add"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "g $ h(x)",
            FunctionCall(
                Variable("$"),
                [Variable("g"), FunctionCall(Variable("h"), [Variable("x")])],
            ),
            "expr",
        ),
        (
            "g $ h $ i(x)",
            FunctionCall(
                Variable("$"),
                [
                    Variable("g"),
                    FunctionCall(
                        Variable("$"),
                        [Variable("h"), FunctionCall(Variable("i"), [Variable("x")])],
                    ),
                ],
            ),
            "expr",
        ),
        (
            "x __add__ f __add__ g",
            FunctionCall(
                Variable("add"),
                [
                    Variable("x"),
                    FunctionCall(Variable("add"), [Variable("f"), Variable("g")]),
                ],
            ),
            "expr",
        ),
        (
            "x |> f |> g",
            FunctionCall(
                Variable("|>"),
                [
                    FunctionCall(Variable("|>"), [Variable("x"), Variable("f")]),
                    Variable("g"),
                ],
            ),
            "expr",
        ),
        (
            "(h @ g @ f)(x)",
            FunctionCall(
                FunctionCall(
                    Variable("@"),
                    [Variable("h"), FunctionCall(Variable("@"), [Variable("g"), Variable("f")])],
                ),
                [
                    Variable("x"),
                ],
            ),
            "expr",
        ),
        (
            "3 :: 4 :: t",
            FunctionCall(
                Variable("::"),
                [Integer(3), FunctionCall(Variable("::"), [Integer(4), Variable("t")])],
            ),
            "expr",
        ),
        (
            "3 == 4",
            FunctionCall(
                Variable("=="),
                [Integer(3), Integer(4)],
            ),
            "expr",
        ),
        (
            "3 == 4 == 5",
            None,
            "expr",
        ),
        (
            "(3 == 4) == (5 == 6)",
            FunctionCall(
                Variable("=="),
                [
                    FunctionCall(
                        Variable("=="),
                        [Integer(3), Integer(4)],
                    ),
                    FunctionCall(
                        Variable("=="),
                        [Integer(5), Integer(6)],
                    ),
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
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [Typename("T")])),
            "assignment",
        ),
        (
            "a<T> = f<T>",
            Assignment(Assignee("a", ["T"]), GenericVariable("f", [Typename("T")])),
            "assignment",
        ),
        (
            "a<T,> = t<T,>",
            Assignment(Assignee("a", ["T"]), GenericVariable("t", [Typename("T")])),
            "assignment",
        ),
        (
            "a<T,U> = -4",
            Assignment(Assignee("a", ["T", "U"]), Integer(-4)),
            "assignment",
        ),
        (
            "a<T,U> = f<U,T>",
            Assignment(
                Assignee("a", ["T", "U"]),
                GenericVariable("f", [Typename("U"), Typename("T")]),
            ),
            "assignment",
        ),
        (
            "a<T,U,> = 0",
            Assignment(Assignee("a", ["T", "U"]), Integer(0)),
            "assignment",
        ),
        ("{5}", Block([], Integer(5)), "block"),
        ("{}", None, "block"),
        (
            "{a = -9; 8}",
            Block([Assignment(Assignee("a", []), Integer(-9))], Integer(8)),
            "block",
        ),
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
                        GenericVariable("x", [Typename("T"), Typename("T")]),
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
                FunctionCall(Variable(">"), [Variable("x"), Integer(0)]),
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
        ("x.0", ElementAccess(Variable("x"), 0), "expr"),
        ("(a, b).1", ElementAccess(TupleExpression([Variable("a"), Variable("b")]), 1), "expr"),
        ("x.-1", None, "expr"),
        ("x.b", None, "expr"),
        ("x.0.(4)", ElementAccess(ElementAccess(Variable("x"), 0), 4), "expr"),
        (
            "x.0.4+1",
            FunctionCall(
                Variable("+"), [ElementAccess(ElementAccess(Variable("x"), 0), 4), Integer(1)]
            ),
            "expr",
        ),
        ("x.0.(4+1)", None, "expr"),
        ("() -> () { () }", FunctionDef([], TupleType([]), Block([], TupleExpression([]))), "expr"),
        (
            "(x: int) -> int { a = 3; 9 }",
            FunctionDef(
                [TypedAssignee(Assignee("x", []), AtomicType.INT)],
                AtomicType.INT,
                Block([Assignment(Assignee("a", []), Integer(3))], Integer(9)),
            ),
            "expr",
        ),
        (
            "(x: int,) -> int { a = 3; 9 }",
            FunctionDef(
                [TypedAssignee(Assignee("x", []), AtomicType.INT)],
                AtomicType.INT,
                Block([Assignment(Assignee("a", []), Integer(3))], Integer(9)),
            ),
            "expr",
        ),
        (
            "(x: int, y: ()) -> int { a = 3; 9 }",
            FunctionDef(
                [
                    TypedAssignee(Assignee("x", []), AtomicType.INT),
                    TypedAssignee(Assignee("y", []), TupleType([])),
                ],
                AtomicType.INT,
                Block([Assignment(Assignee("a", []), Integer(3))], Integer(9)),
            ),
            "expr",
        ),
        (
            "(x: int, y: (),) -> int { a = 3; 9 }",
            FunctionDef(
                [
                    TypedAssignee(Assignee("x", []), AtomicType.INT),
                    TypedAssignee(Assignee("y", []), TupleType([])),
                ],
                AtomicType.INT,
                Block([Assignment(Assignee("a", []), Integer(3))], Integer(9)),
            ),
            "expr",
        ),
        ("(x: int, y: (),) { a = 3; 9 }", None, "expr"),
        ("(x: int,,) -> bool { a = 3; 9 }", None, "expr"),
        ("(,) -> bool { a = 3; 9 }", None, "expr"),
        ("(x, y: bool) -> bool { a = 3; 9 }", None, "expr"),
        ("(x: int, y: bool) -> bool { a = 3;; 9 }", None, "expr"),
        ("(x: int, y: bool) -> bool { a = 3 }", None, "expr"),
        (
            "typedef tuple (int, int)",
            OpaqueTypeDefinition(
                TypeVariable("tuple"), TupleType([AtomicType.INT, AtomicType.INT])
            ),
            "type_def",
        ),
        (
            "typedef tuple ()",
            OpaqueTypeDefinition(TypeVariable("tuple"), TupleType([])),
            "type_def",
        ),
        (
            "typedef tuple<T> (T, T)",
            OpaqueTypeDefinition(
                GenericTypeVariable("tuple", ["T"]), TupleType([Typename("T"), Typename("T")])
            ),
            "type_def",
        ),
        (
            "typedef tuple<T,U> (F<U>, T)",
            OpaqueTypeDefinition(
                GenericTypeVariable("tuple", ["T", "U"]),
                TupleType([GenericType("F", [Typename("U")]), Typename("T")]),
            ),
            "type_def",
        ),
        (
            "typedef apply<T,U> T<U>",
            OpaqueTypeDefinition(
                GenericTypeVariable("apply", ["T", "U"]), GenericType("T", [Typename("U")])
            ),
            "type_def",
        ),
        (
            "typedef alias<T,> T",
            OpaqueTypeDefinition(GenericTypeVariable("alias", ["T"]), Typename("T")),
            "type_def",
        ),
        (
            "typedef Integer int",
            OpaqueTypeDefinition(TypeVariable("Integer"), AtomicType.INT),
            "type_def",
        ),
        (
            "typedef Integer<> int",
            OpaqueTypeDefinition(TypeVariable("Integer"), AtomicType.INT),
            "type_def",
        ),
        ("typedef None", EmptyTypeDefinition("None"), "type_def"),
        ("typedef None<T>", None, "type_def"),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast
