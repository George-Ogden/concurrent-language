use core::fmt;
use itertools::Itertools;
use std::fmt::Formatter;

use lowering::{
    Assignment, AtomicType, AtomicTypeEnum, Await, Boolean, BuiltIn, ElementAccess, Expression,
    FnType, IfStatement, Integer, MachineType, Statement, Store, TupleType, TypeDef, UnionType,
    Value,
};

type Code = String;

struct Translator {}

impl Translator {
    fn translate_type(type_: &MachineType) -> Code {
        format!("{}", TypeFormatter(type_))
    }
    fn translate_type_defs(type_defs: Vec<TypeDef>) -> Code {
        let type_forward_definitions = type_defs
            .iter()
            .map(|type_def| format!("struct {};", type_def.name));
        let constructor_definitions = type_defs
            .iter()
            .map(|type_def| {
                type_def.constructors.iter().map(|constructor| {
                    format!(
                        "typedef {} {} ;",
                        constructor.1.as_ref().map_or(Code::from("Empty"), |type_| {
                            Translator::translate_type(type_)
                        }),
                        constructor.0
                    )
                })
            })
            .flatten();
        let struct_definitions =
            type_defs.iter().map(|type_def| {
                format!(
                "struct {} {{ using type = {}; type value; {}(type value) : value(value) {{}} }};",
                type_def.name,
                Translator::translate_type(&UnionType(
                    type_def
                        .constructors
                        .iter()
                        .map(|constructor| constructor.0.clone())
                        .collect_vec()
                ).into()),
                type_def.name,
            )
            });
        format!(
            "{} {} {}",
            itertools::join(type_forward_definitions, "\n"),
            itertools::join(constructor_definitions, "\n"),
            itertools::join(struct_definitions, "\n")
        )
    }
    fn translate_builtin(value: BuiltIn) -> Code {
        match value {
            BuiltIn::Integer(Integer { value }) => format!("{}LL", value),
            BuiltIn::Boolean(Boolean { value }) => format!("{}", value),
            BuiltIn::BuiltInFn(name, _) => name,
        }
    }
    fn translate_store(store: Store) -> Code {
        store.id()
    }
    fn translate_value(value: Value) -> Code {
        match value {
            Value::BuiltIn(value) => Translator::translate_builtin(value),
            Value::Store(store) => Translator::translate_store(store),
        }
    }
    fn translate_expression(expression: Expression) -> Code {
        match expression {
            Expression::ElementAccess(ElementAccess { value, idx }) => format!(
                "std::get<{}ULL>({})",
                idx,
                Translator::translate_store(value)
            ),
            Expression::Value(value) => Translator::translate_value(value),
            Expression::Wrap(value) => format!(
                "new LazyConstant<{}>({})",
                Translator::translate_type(&value.type_()),
                Translator::translate_value(value)
            ),
            Expression::Unwrap(store) => format!("{}->value()", Translator::translate_store(store)),
            _ => todo!(),
        }
    }
    fn translate_statements(statements: Vec<Statement>) -> Code {
        statements
            .into_iter()
            .map(Translator::translate_statement)
            .join("\n")
    }
    fn translate_statement(statement: Statement) -> Code {
        match statement {
            Statement::Await(await_) => Translator::translate_await(await_),
            Statement::Assignment(assignment) => Translator::translate_assignment(assignment),
            Statement::IfStatement(if_statement) => {
                Translator::translate_if_statement(if_statement)
            }
        }
    }
    fn translate_await(await_: Await) -> Code {
        let stores = await_.0;
        format!(
            "WorkManager::await({});",
            stores
                .into_iter()
                .map(Translator::translate_store)
                .join(",")
        )
    }
    fn translate_assignment(assignment: Assignment) -> Code {
        let target_code = match assignment.target {
            Store::Register(id, type_) => format!("{} {}", Translator::translate_type(&type_), id),
            Store::Memory(id, _) => format!("{}", id),
        };
        let value_code = Translator::translate_expression(assignment.value);
        format!("{} = {};", target_code, value_code)
    }
    fn translate_if_statement(if_statement: IfStatement) -> Code {
        Code::new()
    }
}

struct TypeFormatter<'a>(&'a MachineType);
impl fmt::Display for TypeFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.0 {
            MachineType::AtomicType(AtomicType(atomic)) => match atomic {
                AtomicTypeEnum::INT => write!(f, "Int"),
                AtomicTypeEnum::BOOL => write!(f, "Bool"),
            },
            MachineType::TupleType(TupleType(types)) => {
                write!(f, "TupleT<{}>", TypesFormatter(types))
            }
            MachineType::FnType(FnType(args, ret)) => {
                write!(
                    f,
                    "FnT<{}>",
                    TypesFormatter(
                        &std::iter::once(*ret.clone())
                            .chain(args.clone().into_iter())
                            .collect()
                    )
                )
            }
            MachineType::UnionType(UnionType(type_names)) => {
                write!(f, "VariantT<{}>", type_names.join(","))
            }
            MachineType::NamedType(name) => write!(f, "{}*", name),
            MachineType::Lazy(type_) => write!(f, "Lazy<{}>*", TypeFormatter(&**type_)),
        }
    }
}

struct TypesFormatter<'a>(&'a Vec<MachineType>);
impl fmt::Display for TypesFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            &self
                .0
                .iter()
                .map(|machine_type| format!("{}", TypeFormatter(machine_type)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use lowering::{Id, Name};
    use regex::Regex;
    use test_case::test_case;

    fn normalize_code(code: Code) -> Code {
        let regex = Regex::new(r"((^|[^[:space:]])[[:space:]]+([^[:space:][:word:]]|$))|((^|[^[:space:][:word:]])[[:space:]]+([^[:space:]]|$))")
        .unwrap();

        let mut result = code;
        let mut code = Code::new();
        while result != code {
            code = result;
            result = regex.replace_all(&*code, "${2}${5}${3}${6}").to_string();
        }

        return result;
    }

    fn assert_eq_code(code1: Code, code2: Code) -> () {
        assert_eq!(normalize_code(code1), normalize_code(code2));
    }

    #[test_case(
        "a = 3",
        "a=3";
        "space replacement"
    )]
    #[test_case(
        "int x",
        "int x";
        "no replacement"
    )]
    #[test_case(
        "\t3 + 4",
        "3+4";
        "tab replacement"
    )]
    #[test_case(
        "8+ 5 ",
        "8+5";
        "end replacement"
    )]
    #[test_case(
        "3\n4",
        "3\n4";
        "newline non-replacement"
    )]
    #[test_case(
        "3\n-8",
        "3-8";
        "newline replacement"
    )]
    fn test_code_normalization(code: &str, expected: &str) {
        assert_eq!(normalize_code(Code::from(code)), Code::from(expected))
    }

    #[test_case(
        AtomicType(AtomicTypeEnum::INT).into(),
        "Int";
        "atomic int"
    )]
    #[test_case(
        AtomicType(AtomicTypeEnum::BOOL).into(),
        "Bool";
        "atomic bool"
    )]
    #[test_case(
        TupleType(Vec::new()).into(),
        "TupleT<>";
        "empty tuple type"
    )]
    #[test_case(
        TupleType(vec![AtomicType(AtomicTypeEnum::INT).into()]).into(),
        "TupleT<Int>";
        "singleton tuple type"
    )]
    #[test_case(
        TupleType(vec![
            AtomicType(AtomicTypeEnum::INT).into(),
            AtomicType(AtomicTypeEnum::BOOL).into()
        ]).into(),
        "TupleT<Int,Bool>";
        "double tuple type"
    )]
    #[test_case(
        TupleType(vec![
            TupleType(vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::BOOL).into()
            ]).into(),
            TupleType(Vec::new()).into(),
        ]).into(),
        "TupleT<TupleT<Int,Bool>,TupleT<>>";
        "nested tuple type"
    )]
    #[test_case(
        FnType(Vec::new(), Box::new(TupleType(Vec::new()).into())).into(),
        "FnT<TupleT<>>";
        "unit fn type"
    )]
    #[test_case(
        FnType(
            vec![AtomicType(AtomicTypeEnum::INT).into()],
            Box::new(AtomicType(AtomicTypeEnum::INT).into())
        ).into(),
        "FnT<Int,Int>";
        "int identity fn"
    )]
    #[test_case(
        FnType(
            vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::INT).into()
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        ).into(),
        "FnT<Bool,Int,Int>";
        "int comparison fn"
    )]
    #[test_case(
        FnType(
            vec![
                FnType(
                    vec![
                        AtomicType(AtomicTypeEnum::INT).into()
                    ],
                    Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
                ).into(),
                AtomicType(AtomicTypeEnum::INT).into()
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        ).into(),
        "FnT<Bool,FnT<Bool,Int>,Int>";
        "higher order fn"
    )]
    #[test_case(
        UnionType(
            vec![
                Name::from("Twoo"),
                Name::from("Faws"),
            ]
        ).into(),
        "VariantT<Twoo,Faws>";
        "bull type"
    )]
    #[test_case(
        UnionType(
            vec![
                Name::from("Wrapper"),
            ]
        ).into(),
        "VariantT<Wrapper>";
        "int wrapper variant"
    )]
    #[test_case(
        UnionType(vec![Name::from("Cons_Int"), Name::from("Nil_Int")]).into(),
        "VariantT<Cons_Int,Nil_Int>";
        "list int type"
    )]
    #[test_case(
        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
        "Lazy<Int>*";
        "lazy int type"
    )]
    #[test_case(
        MachineType::Lazy(Box::new(
            TupleType(vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::BOOL).into()
            ]).into()
        )),
        "Lazy<TupleT<Int,Bool>>*";
        "lazy tuple type"
    )]
    fn test_type_translation(type_: MachineType, expected: &str) {
        let code = Translator::translate_type(&type_);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        TypeDef{
            name: Name::from("Bull"),
            constructors: vec![
                (Name::from("Twoo"), None),
                (Name::from("Faws"), None)
            ]
        },
        "struct Bull; typedef Empty Twoo; typedef Empty Faws; struct Bull { using type = VariantT<Twoo, Faws>; type value; Bull(type value) : value(value) {} };";
        "bull union"
    )]
    #[test_case(
        TypeDef{
            name: Name::from("EitherIntBool"),
            constructors: vec![
                (
                    Name::from("Left_IntBool"),
                    Some(
                        AtomicType(AtomicTypeEnum::INT).into(),
                    )
                ),
                (
                    Name::from("Right_IntBool"),
                    Some(
                        AtomicType(AtomicTypeEnum::BOOL).into(),
                    )
                ),
            ]
        },
        "struct EitherIntBool; typedef Int Left_IntBool; typedef Bool Right_IntBool; struct EitherIntBool { using type = VariantT<Left_IntBool, Right_IntBool>; type value; EitherIntBool(type value) : value(value) {} };";
        "either int bool"
    )]
    #[test_case(
        TypeDef{
            name: Name::from("ListInt"),
            constructors: vec![
                (
                    Name::from("Cons_Int"),
                    Some(TupleType(vec![
                        AtomicType(AtomicTypeEnum::INT).into(),
                        MachineType::NamedType(Name::from("ListInt"))
                    ]).into())
                ),
                (Name::from("Nil_Int"), None)
            ]
        },
        "struct ListInt; typedef TupleT<Int, ListInt *> Cons_Int; typedef Empty Nil_Int; struct ListInt { using type = VariantT<Cons_Int, Nil_Int>; type value; ListInt(type value) : value(value) {} };";
        "list int"
    )]
    fn test_typedef_translations(type_def: TypeDef, expected: &str) {
        let code = Translator::translate_type_defs(vec![type_def]);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        vec![
            TypeDef{
                name: Name::from("Expression"),
                constructors: vec![
                    (
                        Name::from("Basic"),
                        Some(AtomicType(AtomicTypeEnum::INT).into())
                    ),
                    (
                        Name::from("Complex"),
                        Some(TupleType(
                            vec![
                                MachineType::NamedType(Name::from("Value")),
                                MachineType::NamedType(Name::from("Value")),
                            ]
                        ).into())
                    ),
                ]
            },
            TypeDef{
                name: Name::from("Value"),
                constructors: vec![
                    (
                        Name::from("None"),
                        None
                    ),
                    (
                        Name::from("Some"),
                        Some(MachineType::NamedType(Name::from("Expression")))
                    ),
                ]
            }
        ],
        "struct Expression; struct Value; typedef Int Basic; typedef TupleT<Value*, Value*> Complex; typedef Empty None; typedef Expression* Some; struct Expression { using type = VariantT<Basic,Complex>; type value; Expression(type value) : value(value) {} }; struct Value { using type = VariantT<None,Some>; type value; Value(type value) : value(value) {} };";
        "mutually recursive types"
    )]
    fn test_typedefs_translations(type_defs: Vec<TypeDef>, expected: &str) {
        let code = Translator::translate_type_defs(type_defs);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Integer{value: 24}.into(),
        "24LL";
        "integer translation"
    )]
    #[test_case(
        Integer{value: -24}.into(),
        "-24LL";
        "negative integer translation"
    )]
    #[test_case(
        Integer{value: 0}.into(),
        "0LL";
        "zero translation"
    )]
    #[test_case(
        Integer{value: 10000000000009}.into(),
        "10000000000009LL";
        "large integer translation"
    )]
    #[test_case(
        Boolean{value: true}.into(),
        "true";
        "true translation"
    )]
    #[test_case(
        Boolean{value: false}.into(),
        "false";
        "false translation"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Plus__BuiltIn"),
            FnType(
                vec![
                    AtomicType(AtomicTypeEnum::INT).into(),
                    AtomicType(AtomicTypeEnum::INT).into()
                ],
                Box::new(AtomicType(AtomicTypeEnum::INT).into())
            ).into()
        ),
        "Plus__BuiltIn";
        "builtin plus translation"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_GE__BuiltIn"),
            FnType(
                vec![
                    AtomicType(AtomicTypeEnum::INT).into(),
                    AtomicType(AtomicTypeEnum::INT).into()
                ],
                Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
            ).into()
        ),
        "Comparison_GE__BuiltIn";
        "builtin greater than or equal to translation"
    )]
    fn test_builtin_translation(value: BuiltIn, expected: &str) {
        let code = Translator::translate_builtin(value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Store::Memory(Id::from("x"), AtomicType(AtomicTypeEnum::BOOL).into()),
        "x";
        "memory translation"
    )]
    #[test_case(
        Store::Register(Id::from("bar"), AtomicType(AtomicTypeEnum::BOOL).into()),
        "bar";
        "register translation"
    )]
    fn test_store_translation(store: Store, expected: &str) {
        let code = Translator::translate_store(store);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Store::Register(
            Id::from("baz"),
            FnType(
                vec![AtomicType(AtomicTypeEnum::INT).into()],
                Box::new(AtomicType(AtomicTypeEnum::INT).into())
            ).into(),
        ).into(),
        "baz";
        "value store translation"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_LT__BuiltIn"),
            FnType(
                vec![
                    AtomicType(AtomicTypeEnum::INT).into(),
                    AtomicType(AtomicTypeEnum::INT).into()
                ],
                Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
            ).into()
        ).into(),
        "Comparison_LT__BuiltIn";
        "builtin function translation"
    )]
    #[test_case(
        BuiltIn::Integer(Integer{value: -1}).into(),
        "-1LL";
        "builtin integer translation"
    )]
    fn test_value_translation(value: Value, expected: &str) {
        let code = Translator::translate_value(value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Value::BuiltIn(BuiltIn::Integer(Integer{value: -1}).into()).into(),
        "-1LL";
        "index access"
    )]
    #[test_case(
        ElementAccess{
            value: Store::Register(
                Name::from("tuple"),
                TupleType(vec![AtomicType(AtomicTypeEnum::INT).into(), AtomicType(AtomicTypeEnum::INT).into()]).into()
            ).into(),
            idx: 1
        }.into(),
        "std::get<1ULL>(tuple)";
        "tuple index access"
    )]
    fn test_expression_translation(expression: Expression, expected: &str) {
        let code = Translator::translate_expression(expression);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Assignment {
            target: Store::Register(Id::from("x"), AtomicType(AtomicTypeEnum::INT).into()).into(),
            value: Value::BuiltIn(Integer{value: 5}.into()).into()
        },
        "Int x = 5LL;";
        "integer assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("x"), AtomicType(AtomicTypeEnum::INT).into()),
            value: ElementAccess{
                value: Store::Register(
                    Name::from("tuple"),
                    TupleType(vec![AtomicType(AtomicTypeEnum::INT).into(), AtomicType(AtomicTypeEnum::INT).into()]).into()
                ).into(),
                idx: 0
            }.into(),

        },
        "Int x = std::get<0ULL>(tuple);";
        "tuple access assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("y"), AtomicType(AtomicTypeEnum::BOOL).into()).into(),
            value: Value::BuiltIn(Boolean{value: true}.into()).into(),

        },
        "y = true;";
        "boolean assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("y"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
            value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into())),

        },
        "Lazy<Bool>* y = new LazyConstant<Bool>(true);";
        "wrapping constant"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(
                Id::from("g"),
                MachineType::Lazy(
                    Box::new(
                        FnType(
                            vec![AtomicType(AtomicTypeEnum::INT).into()],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ).into()
                    )
                )
            ),
            value: Expression::Wrap(Store::Register(
                Id::from("f"),
                FnType(
                    vec![AtomicType(AtomicTypeEnum::INT).into()],
                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                ).into()
            ).into()),
        },
        "g = new LazyConstant<FnT<Int,Int>>(f);";
        "wrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(
                Id::from("w"),
                FnType(
                    vec![AtomicType(AtomicTypeEnum::INT).into()],
                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                ).into()
            ).into(),
            value: Expression::Unwrap(
                Store::Memory(
                    Id::from("g"),
                    MachineType::Lazy(
                        Box::new(
                            FnType(
                                vec![AtomicType(AtomicTypeEnum::INT).into()],
                                Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                            ).into()
                        )
                    )
                )
            ),
        },
        "w = g->value();";
        "unwrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("y"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
            value: Expression::Unwrap(
                Store::Memory(
                    Id::from("t"),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))
                )
            ),
        },
        "y = t->value();";
        "unwrapping boolean from variable"
    )]
    fn test_assignment_translation(assignment: Assignment, expected: &str) {
        let code = Translator::translate_assignment(assignment);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Await(vec![Store::Memory(Id::from("z"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into())))]).into(),
        "WorkManager::await(z);";
        "await for memory"
    )]
    #[test_case(
        Await(vec![
            Store::Register(
                Id::from("z"),
                MachineType::Lazy(Box::new(FnType(
                    vec![AtomicType(AtomicTypeEnum::INT).into()],
                    Box::new(AtomicType(AtomicTypeEnum::INT).into())
                ).into())),
            ),
            Store::Register(
                Id::from("x"),
                MachineType::Lazy(Box::new(
                    AtomicType(AtomicTypeEnum::INT).into()
                )),
            ),
        ]).into(),
        "WorkManager::await(z,x);";
        "await for registers"
    )]
    fn test_statement_translation(statement: Statement, expected: &str) {
        let code = Translator::translate_statement(statement);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        {
            let t = Store::Register(
                Id::from("t"),
                MachineType::Lazy(Box::new(TupleType(
                    vec![AtomicType(AtomicTypeEnum::INT).into(), AtomicType(AtomicTypeEnum::INT).into()],
                ).into())),
            );
            let tuple = Store::Register(
                Id::from("tuple"),
                TupleType(
                    vec![AtomicType(AtomicTypeEnum::INT).into(), AtomicType(AtomicTypeEnum::INT).into()],
                ).into(),
            );
            let x = Store::Register(
                Id::from("x"),
                AtomicType(AtomicTypeEnum::INT).into(),
            );
            vec![
                Await(vec![
                    t.clone(),
                ]).into(),
                Assignment {
                    target: tuple.clone(),
                    value: Expression::Unwrap(
                        t
                    ),
                }.into(),
                Assignment {
                    target: x,
                    value: ElementAccess{
                        value: tuple,
                        idx: 1
                    }.into(),
                }.into()
            ]
        },
        "WorkManager::await(t); TupleT<Int,Int> tuple = t->value(); Int x = std::get<1ULL>(tuple);";
        "tuple access"
    )]
    fn test_statements_translation(statements: Vec<Statement>, expected: &str) {
        let code = Translator::translate_statements(statements);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }
}
