from parser import Parser
from typing import Optional

import pytest
from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    Block,
    Boolean,
    Constructor,
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
    "code,node,target",
    [
        ("int", AtomicType.INT, "type_instance"),
        ("bool", AtomicType.BOOL, "type_instance"),
        ("(int)", AtomicType.INT, "type_instance"),
        ("((int))", AtomicType.INT, "type_instance"),
        ("foo", Typename("foo"), "type_instance"),
        ("foo.<int>", GenericType("foo", [AtomicType.INT]), "type_instance"),
        ("foo.<int,>", GenericType("foo", [AtomicType.INT]), "type_instance"),
        (
            "foo.<int,bool>",
            GenericType("foo", [AtomicType.INT, AtomicType.BOOL]),
            "type_instance",
        ),
        (
            "foo.<bar.<int>,bool>",
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
            FunctionType([AtomicType.INT, AtomicType.BOOL], AtomicType.INT),
            "type_instance",
        ),
        (
            "(int,bool,)->int",
            FunctionType([AtomicType.INT, AtomicType.BOOL], AtomicType.INT),
            "type_instance",
        ),
        (
            "(int,)->int",
            FunctionType([AtomicType.INT], AtomicType.INT),
            "type_instance",
        ),
        ("(int)->int", FunctionType([AtomicType.INT], AtomicType.INT), "type_instance"),
        ("int->int", FunctionType([AtomicType.INT], AtomicType.INT), "type_instance"),
        ("()->()", FunctionType([], TupleType([])), "type_instance"),
        (
            "int->bool->()",
            FunctionType([AtomicType.INT], FunctionType([AtomicType.BOOL], TupleType([]))),
            "type_instance",
        ),
        (
            "(int->bool->())",
            FunctionType([AtomicType.INT], FunctionType([AtomicType.BOOL], TupleType([]))),
            "type_instance",
        ),
        (
            "int->(bool->())",
            FunctionType([AtomicType.INT], FunctionType([AtomicType.BOOL], TupleType([]))),
            "type_instance",
        ),
        (
            "(int)->(bool->())",
            FunctionType([AtomicType.INT], FunctionType([AtomicType.BOOL], TupleType([]))),
            "type_instance",
        ),
        (
            "(int->bool)->()",
            FunctionType([FunctionType([AtomicType.INT], AtomicType.BOOL)], TupleType([])),
            "type_instance",
        ),
        (
            "(int->(int,),)->(())",
            FunctionType(
                [FunctionType([AtomicType.INT], TupleType([AtomicType.INT]))],
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
        ("x", Var("x"), "expr"),
        ("foo", Var("foo"), "expr"),
        ("r2d2", Var("r2d2"), "expr"),
        ("f'", Var("f'"), "expr"),
        ("g''", Var("g''"), "expr"),
        ("f'f", None, "expr"),
        ("__^__", Var("^"), "expr"),
        ("___^__", None, "expr"),
        ("__^^^__", Var("^^^"), "expr"),
        ("map.<int>", GenericVariable("map", [AtomicType.INT]), "expr"),
        ("map.<int,>", GenericVariable("map", [AtomicType.INT]), "expr"),
        ("map.<T>", GenericVariable("map", [Typename("T")]), "expr"),
        (
            "map.<f.<int>>",
            GenericVariable("map", [GenericType("f", [AtomicType.INT])]),
            "expr",
        ),
        (
            "map.<f.<g.<T>>>",
            GenericVariable("map", [GenericType("f", [GenericType("g", [Typename("T")])])]),
            "expr",
        ),
        (
            "map.<int,bool>",
            GenericVariable("map", [AtomicType.INT, AtomicType.BOOL]),
            "expr",
        ),
        (
            "map.<(int,int)>",
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
        ("3 + 4", FunctionCall(Var("+"), [Integer(3), Integer(4)]), "expr"),
        ("3 * 4", FunctionCall(Var("*"), [Integer(3), Integer(4)]), "expr"),
        ("3 &&$& 4", FunctionCall(Var("&&$&"), [Integer(3), Integer(4)]), "expr"),
        (
            "3 __add__ 4",
            FunctionCall(Var("add"), [Integer(3), Integer(4)]),
            "expr",
        ),
        ("3 ____ 4", None, "expr"),
        ("3 __^__ 4", None, "expr"),
        ("3 _____ 4", FunctionCall(Var("_"), [Integer(3), Integer(4)]), "expr"),
        ("3 __f'__ 4", FunctionCall(Var("f'"), [Integer(3), Integer(4)]), "expr"),
        ("3 __f''__ 4", FunctionCall(Var("f''"), [Integer(3), Integer(4)]), "expr"),
        ("3 ______ 4", FunctionCall(Var("__"), [Integer(3), Integer(4)]), "expr"),
        ("3 _______ 4", FunctionCall(Var("___"), [Integer(3), Integer(4)]), "expr"),
        ("3 ________ 4", FunctionCall(Var("____"), [Integer(3), Integer(4)]), "expr"),
        (
            "3 + 4 + 5",
            FunctionCall(
                Var("+"),
                [FunctionCall(Var("+"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "3 * 4 + 5",
            FunctionCall(
                Var("+"),
                [FunctionCall(Var("*"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "3 + 4 + 5 + 6",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            FunctionCall(Var("+"), [Integer(3), Integer(4)]),
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
                Var("add"),
                [
                    Integer(3),
                    FunctionCall(
                        Var("add"),
                        [
                            Integer(4),
                            FunctionCall(Var("add"), [Integer(5), Integer(6)]),
                        ],
                    ),
                ],
            ),
            "expr",
        ),
        (
            "3 + 4 * 5",
            FunctionCall(
                Var("+"),
                [Integer(3), FunctionCall(Var("*"), [Integer(4), Integer(5)])],
            ),
            "expr",
        ),
        (
            "(3 + 4) * 5",
            FunctionCall(
                Var("*"),
                [FunctionCall(Var("+"), [Integer(3), Integer(4)]), Integer(5)],
            ),
            "expr",
        ),
        (
            "2 * 3 + 4 * 5",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(Var("*"), [Integer(2), Integer(3)]),
                    FunctionCall(Var("*"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "3 __mul__ 4 + 5",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(Var("mul"), [Integer(3), Integer(4)]),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 * 3 + 4 + 5",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            FunctionCall(Var("*"), [Integer(2), Integer(3)]),
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
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            Integer(2),
                            Integer(3),
                        ],
                    ),
                    FunctionCall(Var("*"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 * 4 + 5",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            Integer(2),
                            FunctionCall(Var("*"), [Integer(3), Integer(4)]),
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
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            Integer(2),
                            FunctionCall(Var("mul"), [Integer(3), Integer(4)]),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 + 3 <!> 4 + 5",
            FunctionCall(
                Var("+"),
                [
                    FunctionCall(
                        Var("+"),
                        [
                            Integer(2),
                            FunctionCall(Var("<!>"), [Integer(3), Integer(4)]),
                        ],
                    ),
                    Integer(5),
                ],
            ),
            "expr",
        ),
        (
            "2 __add__ 3 <!> 4 __add__ 5",
            FunctionCall(
                Var("<!>"),
                [
                    FunctionCall(Var("add"), [Integer(2), Integer(3)]),
                    FunctionCall(Var("add"), [Integer(4), Integer(5)]),
                ],
            ),
            "expr",
        ),
        (
            "g $ h(x)",
            FunctionCall(
                Var("$"),
                [Var("g"), FunctionCall(Var("h"), [Var("x")])],
            ),
            "expr",
        ),
        (
            "g $ h $ i(x)",
            FunctionCall(
                Var("$"),
                [
                    Var("g"),
                    FunctionCall(
                        Var("$"),
                        [Var("h"), FunctionCall(Var("i"), [Var("x")])],
                    ),
                ],
            ),
            "expr",
        ),
        (
            "x __add__ f __add__ g",
            FunctionCall(
                Var("add"),
                [
                    Var("x"),
                    FunctionCall(Var("add"), [Var("f"), Var("g")]),
                ],
            ),
            "expr",
        ),
        (
            "x |> f |> g",
            FunctionCall(
                Var("|>"),
                [
                    FunctionCall(Var("|>"), [Var("x"), Var("f")]),
                    Var("g"),
                ],
            ),
            "expr",
        ),
        (
            "(h @ g @ f)(x)",
            FunctionCall(
                FunctionCall(
                    Var("@"),
                    [
                        Var("h"),
                        FunctionCall(Var("@"), [Var("g"), Var("f")]),
                    ],
                ),
                [
                    Var("x"),
                ],
            ),
            "expr",
        ),
        (
            "3 :: 4 :: t",
            FunctionCall(
                Var("::"),
                [Integer(3), FunctionCall(Var("::"), [Integer(4), Var("t")])],
            ),
            "expr",
        ),
        (
            "3 == 4",
            FunctionCall(
                Var("=="),
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
                Var("=="),
                [
                    FunctionCall(
                        Var("=="),
                        [Integer(3), Integer(4)],
                    ),
                    FunctionCall(
                        Var("=="),
                        [Integer(5), Integer(6)],
                    ),
                ],
            ),
            "expr",
        ),
        (
            "foo()",
            FunctionCall(Var("foo"), []),
            "expr",
        ),
        (
            "foo(4,)",
            FunctionCall(Var("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4)",
            FunctionCall(Var("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4,5)",
            FunctionCall(Var("foo"), [Integer(4), Integer(5)]),
            "expr",
        ),
        (
            "foo(4,5,)",
            FunctionCall(Var("foo"), [Integer(4), Integer(5)]),
            "expr",
        ),
        (
            "(foo)(4)",
            FunctionCall(Var("foo"), [Integer(4)]),
            "expr",
        ),
        (
            "__^__(4)",
            FunctionCall(Var("^"), [Integer(4)]),
            "expr",
        ),
        (
            "foo(4)(-5,0)",
            FunctionCall(FunctionCall(Var("foo"), [Integer(4)]), [Integer(-5), Integer(0)]),
            "expr",
        ),
        (
            "foo(4)(a)(-5,bar(true))",
            FunctionCall(
                FunctionCall(FunctionCall(Var("foo"), [Integer(4)]), [Var("a")]),
                [Integer(-5), FunctionCall(Var("bar"), [Boolean(True)])],
            ),
            "expr",
        ),
        (
            "a = 3",
            Assignment(ParametricAssignee(Assignee("a"), []), Integer(3)),
            "assignment",
        ),
        (
            "__a__ = 3",
            Assignment(ParametricAssignee(Assignee("__a__"), []), Integer(3)),
            "assignment",
        ),
        (
            "__&&__ = 3",
            Assignment(ParametricAssignee(Assignee("&&"), []), Integer(3)),
            "assignment",
        ),
        (
            "__>__ = 3",
            Assignment(ParametricAssignee(Assignee(">"), []), Integer(3)),
            "assignment",
        ),
        (
            "__>__ = 3",
            Assignment(ParametricAssignee(Assignee(">"), []), Integer(3)),
            "assignment",
        ),
        (
            "__$__ = 3",
            Assignment(ParametricAssignee(Assignee("$"), []), Integer(3)),
            "assignment",
        ),
        ("__$ $__ = 3", None, "assignment"),
        ("a == 3", None, "assignment"),
        ("0 = 3", None, "assignment"),
        ("__=__ = 4", None, "assignment"),
        ("__.__ = 4", None, "assignment"),
        (
            "__==__ = 4",
            Assignment(ParametricAssignee(Assignee("=="), []), Integer(4)),
            "assignment",
        ),
        (
            "a0 = 0",
            Assignment(ParametricAssignee(Assignee("a0"), []), Integer(0)),
            "assignment",
        ),
        (
            "_ = 0",
            Assignment(ParametricAssignee(Assignee("_"), []), Integer(0)),
            "assignment",
        ),
        (
            "__ = 0",
            Assignment(ParametricAssignee(Assignee("__"), []), Integer(0)),
            "assignment",
        ),
        (
            "___ = 0",
            Assignment(ParametricAssignee(Assignee("___"), []), Integer(0)),
            "assignment",
        ),
        (
            "____ = 0",
            Assignment(ParametricAssignee(Assignee("____"), []), Integer(0)),
            "assignment",
        ),
        (
            "_____ = 0",
            Assignment(ParametricAssignee(Assignee("_____"), []), Integer(0)),
            "assignment",
        ),
        (
            "a<T> = f.<T>",
            Assignment(
                ParametricAssignee(Assignee("a"), ["T"]),
                GenericVariable("f", [Typename("T")]),
            ),
            "assignment",
        ),
        (
            "a<T> = f.<T>",
            Assignment(
                ParametricAssignee(Assignee("a"), ["T"]),
                GenericVariable("f", [Typename("T")]),
            ),
            "assignment",
        ),
        (
            "a<T,> = t.<T,>",
            Assignment(
                ParametricAssignee(Assignee("a"), ["T"]),
                GenericVariable("t", [Typename("T")]),
            ),
            "assignment",
        ),
        (
            "a<T,U> = -4",
            Assignment(ParametricAssignee(Assignee("a"), ["T", "U"]), Integer(-4)),
            "assignment",
        ),
        (
            "a<T,U> = f.<U,T>",
            Assignment(
                ParametricAssignee(Assignee("a"), ["T", "U"]),
                GenericVariable("f", [Typename("U"), Typename("T")]),
            ),
            "assignment",
        ),
        (
            "a<T,U,> = 0",
            Assignment(ParametricAssignee(Assignee("a"), ["T", "U"]), Integer(0)),
            "assignment",
        ),
        ("{5}", Block([], Integer(5)), "block"),
        ("{}", None, "block"),
        (
            "{a = -9; 8}",
            Block(
                [Assignment(ParametricAssignee(Assignee("a"), []), Integer(-9))],
                Integer(8),
            ),
            "block",
        ),
        ("{a = -9}", None, "block"),
        ("{a = -9;}", None, "block"),
        ("{; 8}", None, "block"),
        ("{w = x;; 8}", None, "block"),
        (
            "{w = x;y<T> = x.<T,T>; -8}",
            Block(
                [
                    Assignment(ParametricAssignee(Assignee("w"), []), Var("x")),
                    Assignment(
                        ParametricAssignee(Assignee("y"), ["T"]),
                        GenericVariable("x", [Typename("T"), Typename("T")]),
                    ),
                ],
                Integer(-8),
            ),
            "block",
        ),
        (
            "{w = x; ()}",
            Block(
                [Assignment(ParametricAssignee(Assignee("w"), []), Var("x"))],
                TupleExpression([]),
            ),
            "block",
        ),
        (
            "if (g) { 1 } else { 2 }",
            IfExpression(Var("g"), Block([], Integer(1)), Block([], Integer(2))),
            "expr",
        ),
        (
            "if (x > 0) { x = 0; true } else { x = 1; false }",
            IfExpression(
                FunctionCall(Var(">"), [Var("x"), Integer(0)]),
                Block(
                    [Assignment(ParametricAssignee(Assignee("x"), []), Integer(0))],
                    Boolean(True),
                ),
                Block(
                    [Assignment(ParametricAssignee(Assignee("x"), []), Integer(1))],
                    Boolean(False),
                ),
            ),
            "expr",
        ),
        (
            "match (maybe()) { Some x: { t }, None : { y },}",
            MatchExpression(
                FunctionCall(Var("maybe"), []),
                [
                    MatchBlock([MatchItem("Some", Assignee("x"))], Block([], Var("t"))),
                    MatchBlock([MatchItem("None", None)], Block([], Var("y"))),
                ],
            ),
            "expr",
        ),
        (
            "match (maybe()) { Some x: { t }, None : { y }}",
            MatchExpression(
                FunctionCall(Var("maybe"), []),
                [
                    MatchBlock([MatchItem("Some", Assignee("x"))], Block([], Var("t"))),
                    MatchBlock([MatchItem("None", None)], Block([], Var("y"))),
                ],
            ),
            "expr",
        ),
        (
            "match(()) { Some x | None: { () }, }",
            MatchExpression(
                TupleExpression([]),
                [
                    MatchBlock(
                        [MatchItem("Some", Assignee("x")), MatchItem("None", None)],
                        Block([], TupleExpression([])),
                    ),
                ],
            ),
            "expr",
        ),
        ("x.0", ElementAccess(Var("x"), 0), "expr"),
        (
            "(a, b).1",
            ElementAccess(TupleExpression([Var("a"), Var("b")]), 1),
            "expr",
        ),
        ("x.-1", None, "expr"),
        ("f . g", FunctionCall(Var("."), [Var("f"), Var("g")]), "expr"),
        (
            "(f . g)(x)",
            FunctionCall(
                FunctionCall(Var("."), [Var("f"), Var("g")]),
                [Var("x")],
            ),
            "expr",
        ),
        ("a ... b", FunctionCall(Var("..."), [Var("a"), Var("b")]), "expr"),
        ("a .. b", FunctionCall(Var(".."), [Var("a"), Var("b")]), "expr"),
        ("a .>. b", None, "expr"),
        ("x.b", None, "expr"),
        ("x.0.(4)", None, "expr"),
        (
            "x.0.4+1",
            FunctionCall(
                Var("+"),
                [ElementAccess(ElementAccess(Var("x"), 0), 4), Integer(1)],
            ),
            "expr",
        ),
        ("x.0.(4+1)", None, "expr"),
        (
            "() -> () { () }",
            FunctionDefinition([], TupleType([]), Block([], TupleExpression([]))),
            "expr",
        ),
        (
            "(x: int) -> int { a = 3; 9 }",
            FunctionDefinition(
                [TypedAssignee(Assignee("x"), AtomicType.INT)],
                AtomicType.INT,
                Block(
                    [Assignment(ParametricAssignee(Assignee("a"), []), Integer(3))],
                    Integer(9),
                ),
            ),
            "expr",
        ),
        (
            "(x: int,) -> int { a = 3; 9 }",
            FunctionDefinition(
                [TypedAssignee(Assignee("x"), AtomicType.INT)],
                AtomicType.INT,
                Block(
                    [Assignment(ParametricAssignee(Assignee("a"), []), Integer(3))],
                    Integer(9),
                ),
            ),
            "expr",
        ),
        (
            "(x: int, y: ()) -> int { a = 3; 9 }",
            FunctionDefinition(
                [
                    TypedAssignee(Assignee("x"), AtomicType.INT),
                    TypedAssignee(Assignee("y"), TupleType([])),
                ],
                AtomicType.INT,
                Block(
                    [Assignment(ParametricAssignee(Assignee("a"), []), Integer(3))],
                    Integer(9),
                ),
            ),
            "expr",
        ),
        (
            "(x: int, y: (),) -> int { a = 3; 9 }",
            FunctionDefinition(
                [
                    TypedAssignee(Assignee("x"), AtomicType.INT),
                    TypedAssignee(Assignee("y"), TupleType([])),
                ],
                AtomicType.INT,
                Block(
                    [Assignment(ParametricAssignee(Assignee("a"), []), Integer(3))],
                    Integer(9),
                ),
            ),
            "expr",
        ),
        ("(x: int, y: (),) { a = 3; 9 }", None, "expr"),
        ("(x: int,,) -> bool { a = 3; 9 }", None, "expr"),
        ("(,) -> bool { a = 3; 9 }", None, "expr"),
        ("(x, y: bool) -> bool { a = 3; 9 }", None, "expr"),
        ("(x: int, y: bool) -> bool { a = 3;; 9 }", None, "expr"),
        ("(x: int, y: bool) -> bool { a = 3 }", None, "expr"),
        ("++x", FunctionCall(Var("++"), [Var("x")]), "expr"),
        ("-x", FunctionCall(Var("-"), [Var("x")]), "expr"),
        ("__add__ x", None, "expr"),
        (
            "++ (++x)",
            FunctionCall(Var("++"), [FunctionCall(Var("++"), [Var("x")])]),
            "expr",
        ),
        (
            "++ ++x",
            FunctionCall(Var("++"), [FunctionCall(Var("++"), [Var("x")])]),
            "expr",
        ),
        ("++++x", FunctionCall(Var("++++"), [Var("x")]), "expr"),
        ("Integer{8}", ConstructorCall(Constructor("Integer"), [Integer(8)]), "expr"),
        ("Integer{8,}", ConstructorCall(Constructor("Integer"), [Integer(8)]), "expr"),
        ("Integer{8,9}", ConstructorCall(Constructor("Integer"), [Integer(8), Integer(9)]), "expr"),
        (
            "Integer{8,9,}",
            ConstructorCall(Constructor("Integer"), [Integer(8), Integer(9)]),
            "expr",
        ),
        (
            "Cons.<U>{(f(h),map.<T,U>(f, t))}",
            ConstructorCall(
                GenericConstructor("Cons", [Typename("U")]),
                [
                    TupleExpression(
                        [
                            FunctionCall(Var("f"), [Var("h")]),
                            FunctionCall(
                                GenericVariable("map", [Typename("T"), Typename("U")]),
                                [Var("f"), Var("t")],
                            ),
                        ]
                    )
                ],
            ),
            "expr",
        ),
        ("__^__{8}", None, "expr"),
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
                GenericTypeVariable("tuple", ["T"]),
                TupleType([Typename("T"), Typename("T")]),
            ),
            "type_def",
        ),
        (
            "typedef tuple<T,U> (F.<U>, T)",
            OpaqueTypeDefinition(
                GenericTypeVariable("tuple", ["T", "U"]),
                TupleType([GenericType("F", [Typename("U")]), Typename("T")]),
            ),
            "type_def",
        ),
        (
            "typedef apply<T,U> T.<U>",
            OpaqueTypeDefinition(
                GenericTypeVariable("apply", ["T", "U"]),
                GenericType("T", [Typename("U")]),
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
        (
            "typedef Maybe<T> { Some T | None }",
            UnionTypeDefinition(
                GenericTypeVariable("Maybe", ["T"]),
                [TypeItem("Some", Typename("T")), TypeItem("None", None)],
            ),
            "type_def",
        ),
        (
            "typedef Choice<T, U> { Left T | Right U }",
            UnionTypeDefinition(
                GenericTypeVariable("Choice", ["T", "U"]),
                [
                    TypeItem("Left", Typename("T")),
                    TypeItem("Right", Typename("U")),
                ],
            ),
            "type_def",
        ),
        (
            "typedef Error {Error1|Error2}",
            UnionTypeDefinition(
                TypeVariable("Error"),
                [TypeItem("Error1", None), TypeItem("Error2", None)],
            ),
            "type_def",
        ),
        (
            "typedef Error {Error1}",
            None,
            "type_def",
        ),
        (
            "typedef Error {}",
            None,
            "type_def",
        ),
        (
            "typedef Error<T> {Error1{T} | Error2}",
            None,
            "type_def",
        ),
        (
            "typedef Error<T> {Error1 | Error2}",
            UnionTypeDefinition(
                GenericTypeVariable("Error", ["T"]),
                [TypeItem("Error1", None), TypeItem("Error2", None)],
            ),
            "type_def",
        ),
        (
            "typealias int8 int",
            TransparentTypeDefinition(TypeVariable("int8"), AtomicType.INT),
            "type_alias",
        ),
        (
            "typealias int8 (int,)",
            TransparentTypeDefinition(TypeVariable("int8"), TupleType([AtomicType.INT])),
            "type_alias",
        ),
        (
            "typealias id<T> T -> T",
            TransparentTypeDefinition(
                GenericTypeVariable("id", ["T"]),
                FunctionType([Typename("T")], Typename("T")),
            ),
            "type_alias",
        ),
        (
            "typealias int8<> int",
            TransparentTypeDefinition(TypeVariable("int8"), AtomicType.INT),
            "type_alias",
        ),
        (
            "typealias id<T> (T -> T)",
            TransparentTypeDefinition(
                GenericTypeVariable("id", ["T"]),
                FunctionType([Typename("T")], Typename("T")),
            ),
            "type_alias",
        ),
        ("typealias MaybeInt {Some int | None}", None, "type_alias"),
        ("typealias int", None, "type_alias"),
        (
            "z = -y;",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("z"), []),
                        FunctionCall(Var("-"), [Var("y")]),
                    )
                ],
            ),
            "program",
        ),
        (
            "z = -y",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("z"), []),
                        FunctionCall(Var("-"), [Var("y")]),
                    )
                ],
            ),
            "program",
        ),
        (
            "z = -y; typedef int8 int",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("z"), []),
                        FunctionCall(Var("-"), [Var("y")]),
                    ),
                    OpaqueTypeDefinition(TypeVariable("int8"), AtomicType.INT),
                ],
            ),
            "program",
        ),
        (
            "z = -y; typedef int8 int;",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("z"), []),
                        FunctionCall(Var("-"), [Var("y")]),
                    ),
                    OpaqueTypeDefinition(TypeVariable("int8"), AtomicType.INT),
                ],
            ),
            "program",
        ),
        (
            "z = -y ; typedef int8 int ; ",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("z"), []),
                        FunctionCall(Var("-"), [Var("y")]),
                    ),
                    OpaqueTypeDefinition(TypeVariable("int8"), AtomicType.INT),
                ],
            ),
            "program",
        ),
        (
            "typedef None ",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef /* None */ Nada",
            Program(
                [EmptyTypeDefinition("Nada")],
            ),
            "program",
        ),
        (
            "typedef  None /* Nada",
            None,
            "program",
        ),
        (
            "typedef  None // Nada",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef  None ; // Nada",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef // Nada \n None ;",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef /* Nada \n Not */ None;",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef /* Nada \n Not * / // ;",
            None,
            "program",
        ),
        (
            "typedef /* Nada \n Not */ None // ;",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef /* Nada \n Not */ // None;",
            None,
            "program",
        ),
        (
            "typedef /* Nada \n Not /* */ None;",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "typedef /* Nada \n Not // */ None;",
            Program(
                [EmptyTypeDefinition("None")],
            ),
            "program",
        ),
        (
            "x = 3 /*/ 4 // */",
            Program(
                [Assignment(ParametricAssignee(Assignee("x"), []), Integer(3))],
            ),
            "program",
        ),
        (
            "x = 3 /-/ 4 // */",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("x"), []),
                        FunctionCall(Var("/-/"), [Integer(3), Integer(4)]),
                    )
                ],
            ),
            "program",
        ),
        (
            "x = () -> () { 3 }",
            Program(
                [
                    Assignment(
                        ParametricAssignee(Assignee("x"), []),
                        FunctionDefinition([], TupleType([]), Block([], Integer(3))),
                    )
                ],
            ),
            "program",
        ),
        (
            "x = () -> () { typedef int8 int; 3 }",
            None,
            "program",
        ),
        (
            "x = () -> () { typealias int8 int; 3 }",
            None,
            "program",
        ),
        (
            "x + 3",
            None,
            "program",
        ),
    ],
)
def test_parse(code: str, node: Optional[ASTNode], target: str):
    ast = Parser.parse(code, target=target)
    assert node == ast


def parse_sample():
    with open("sample.txt") as f:
        code = f.read()
    ast = Parser.parse(code, target="program")
    assert ast is not None
