use core::fmt;
use itertools::Itertools;
use std::{collections::HashSet, fmt::Formatter, hash::RandomState};

use lowering::{
    Assignment, AtomicType, AtomicTypeEnum, Await, Block, Boolean, BuiltIn, ClosureInstantiation,
    ConstructorCall, ElementAccess, Expression, FnCall, FnDef, FnType, Id, IfStatement, Integer,
    MachineType, MatchStatement, Statement, Store, TupleExpression, TupleType, TypeDef, UnionType,
    Value,
};

type Code = String;

struct Translator {}

impl Translator {
    fn translate_type(&self, type_: &MachineType) -> Code {
        format!("{}", TypeFormatter(type_))
    }
    fn translate_type_defs(&self, type_defs: Vec<TypeDef>) -> Code {
        let forward_constructor_definitions = type_defs
            .iter()
            .map(|type_def| {
                type_def
                    .constructors
                    .iter()
                    .map(|constructor| format!("struct {};", constructor.0))
            })
            .flatten();
        let type_definitions = type_defs.iter().map(|type_def| {
            let variant_definition = self.translate_type(&MachineType::UnionType(UnionType(
                type_def
                    .constructors
                    .iter()
                    .map(|constructor| constructor.0.clone())
                    .collect_vec(),
            )));
            format!("typedef {variant_definition} {};", type_def.name)
        });
        let constructor_definitions = type_defs
            .iter()
            .map(|type_def| {
                type_def.constructors.iter().map(|constructor| {
                    let fields = match &constructor.1 {
                        Some(type_) => {
                            format!("using type = {}; type value;", self.translate_type(type_))
                        }
                        None => String::new(),
                    };
                    format!("struct {} {{ {fields} }};", constructor.0)
                })
            })
            .flatten();
        format!(
            "{} {} {}",
            itertools::join(forward_constructor_definitions, "\n"),
            itertools::join(type_definitions, "\n"),
            itertools::join(constructor_definitions, "\n"),
        )
    }
    fn translate_builtin(&self, value: BuiltIn) -> Code {
        match value {
            BuiltIn::Integer(Integer { value }) => format!("{value}LL"),
            BuiltIn::Boolean(Boolean { value }) => format!("{value}"),
            BuiltIn::BuiltInFn(name, _) => name,
        }
    }
    fn translate_store(&self, store: Store) -> Code {
        store.id()
    }
    fn translate_block(&self, block: Block) -> Code {
        let statements_code = self.translate_statements(block.statements);
        let MachineType::Lazy(type_) = &block.ret.type_() else {
            panic!("Block has non-lazy return type.")
        };
        let type_code = self.translate_type(&*type_);
        let return_code = format!("return {};", self.translate_store(block.ret));
        format!("new BlockFn<{type_code}>([&]() {{ {statements_code} {return_code} }})")
    }
    fn translate_value(&self, value: Value) -> Code {
        match value {
            Value::BuiltIn(value) => self.translate_builtin(value),
            Value::Store(store) => self.translate_store(store),
            Value::Block(block) => self.translate_block(block),
        }
    }
    fn translate_value_list(&self, values: Vec<Value>) -> Code {
        values
            .into_iter()
            .map(|value| self.translate_value(value))
            .join(", ")
    }
    fn translate_expression(&self, expression: Expression) -> Code {
        match expression {
            Expression::ElementAccess(ElementAccess { value, idx }) => {
                format!("std::get<{idx}ULL>({})", self.translate_store(value))
            }
            Expression::Value(value) => self.translate_value(value),
            Expression::Wrap(value) => format!(
                "new LazyConstant<{}>{{{}}}",
                self.translate_type(&value.type_()),
                self.translate_value(value)
            ),
            Expression::Unwrap(store) => format!("{}->value()", self.translate_store(store)),
            Expression::Reference(store) => format!(
                "new {}{{{}}}",
                self.translate_type(&store.type_()),
                self.translate_store(store)
            ),
            Expression::Dereference(store) => format!("*{}", self.translate_store(store)),
            Expression::TupleExpression(TupleExpression(values)) => {
                format!("std::make_tuple({})", self.translate_value_list(values))
            }
            Expression::ClosureInstantiation(ClosureInstantiation { name, env }) => {
                format!("new {name}{{{}}}", self.translate_value(env))
            }
            e => panic!("{:?} does not translate directly as an expression", e),
        }
    }
    fn translate_await(&self, await_: Await) -> Code {
        let arguments = await_
            .0
            .into_iter()
            .map(|store| self.translate_store(store))
            .join(",");
        format!("WorkManager::await({arguments});")
    }
    fn translate_fn_call(&self, target: Id, fn_call: FnCall) -> Code {
        let fn_initialization_code = match fn_call.fn_ {
            Value::BuiltIn(BuiltIn::BuiltInFn(name, _)) => {
                format!("new {name}{{}};")
            }
            Value::Store(store) => {
                let store_code = self.translate_store(store);
                format!("{store_code}->clone();",)
            }
            Value::Block(block) => {
                format!("{};", self.translate_block(block))
            }
            _ => panic!("Calling invalid function"),
        };
        let args_assignment = format!(
            "{target}->args = std::make_tuple({});",
            self.translate_value_list(fn_call.args)
        );
        format!("{fn_initialization_code} {args_assignment} {target}->call()",)
    }
    fn translate_constructor_call(&self, target: Id, constructor_call: ConstructorCall) -> Code {
        let declaration = format!("{{}};");
        let indexing_code = format!("{target}.tag = {}ULL", constructor_call.idx);
        let value_code = match constructor_call.data {
            None => Code::new(),
            Some((name, value)) => format!(
                "reinterpret_cast<{name}*>(&{target}.value)->value = {};",
                self.translate_value(value)
            ),
        };
        format!("{declaration} {value_code} {indexing_code}")
    }
    fn translate_assignment(&self, assignment: Assignment) -> Code {
        let id = assignment.target.id();
        let value_code = match assignment.value {
            Expression::FnCall(fn_call) => self.translate_fn_call(id, fn_call),
            Expression::ConstructorCall(constructor_call) => {
                self.translate_constructor_call(id, constructor_call)
            }
            value => self.translate_expression(value),
        };
        let id = &assignment.target.id();
        let assignment_code = format!("{id} = {value_code};");
        match assignment.target {
            Store::Register(_, type_) => {
                format!("{} {assignment_code}", self.translate_type(&type_))
            }
            Store::Memory(id, _) => format!("if ({id} == nullptr) {{ {assignment_code} }}"),
            Store::Global(_, _) => assignment_code,
        }
    }
    fn translate_if_statement(&self, if_statement: IfStatement) -> Code {
        let condition_code = self.translate_store(if_statement.condition);
        let if_branch = self.translate_statements(if_statement.branches.0);
        let else_branch = self.translate_statements(if_statement.branches.1);
        format!("if ({condition_code}) {{ {if_branch} }} else {{ {else_branch} }}",)
    }
    fn translate_match_statement(&self, match_statement: MatchStatement) -> Code {
        let MachineType::UnionType(UnionType(types)) = match_statement.expression.type_() else {
            panic!("Matching with non-union type")
        };
        let branches_code = match_statement
            .branches
            .into_iter()
            .enumerate()
            .map(|(i, branch)| {
                let assignment_code = match branch.target {
                    Some(id) => {
                        let type_name = &types[i];
                        let expression_id = &match_statement.expression.id();
                        format!(
                            "{type_name}::type {id} = reinterpret_cast<{type_name}*>(&{expression_id}.value)->value;",
                        )
                    }
                    None => Code::new(),
                };
                let statements_code = self.translate_statements(branch.statements);

                format!(
                    "case {i}ULL : {{ {assignment_code} {statements_code} break; }}",
                )
            })
            .join("\n");
        let expression_code = format!("{}.tag", self.translate_store(match_statement.expression));
        format!("switch ({expression_code}) {{ {branches_code} }}")
    }
    fn translate_statement(&self, statement: Statement) -> Code {
        match statement {
            Statement::Await(await_) => self.translate_await(await_),
            Statement::Assignment(assignment) => self.translate_assignment(assignment),
            Statement::IfStatement(if_statement) => self.translate_if_statement(if_statement),
            Statement::MatchStatement(match_statement) => {
                self.translate_match_statement(match_statement)
            }
        }
    }
    fn translate_statements(&self, statements: Vec<Statement>) -> Code {
        statements
            .into_iter()
            .map(|statement| self.translate_statement(statement))
            .join("\n")
    }
    fn merge_memory_allocations(
        &self,
        memory_allocations: Vec<Vec<MemoryAllocation>>,
    ) -> Vec<MemoryAllocation> {
        let ids = memory_allocations
            .iter()
            .map(|memory_allocations| {
                memory_allocations
                    .iter()
                    .map(|MemoryAllocation(id, _)| id.clone())
                    .collect::<HashSet<_>>()
            })
            .concat();
        let unique_allocations: HashSet<MemoryAllocation, RandomState> = memory_allocations
            .into_iter()
            .map(HashSet::from_iter)
            .concat();
        if ids.len() != unique_allocations.len() {
            panic!("Memory allocations exist with a different size.")
        }
        unique_allocations
            .into_iter()
            .sorted_by_key(|memory_allocation| memory_allocation.0.clone())
            .collect_vec()
    }
    fn find_memory_allocations_from_statements(
        &self,
        statements: &Vec<Statement>,
    ) -> Vec<MemoryAllocation> {
        statements
            .iter()
            .map(|statement| self.find_memory_allocations_from_statement(statement))
            .concat()
    }
    fn find_memory_allocations_from_statement(
        &self,
        statement: &Statement,
    ) -> Vec<MemoryAllocation> {
        let allocations = match statement {
            Statement::Await(_) => Vec::new(),
            Statement::Assignment(Assignment { target, value }) => {
                self.merge_memory_allocations(vec![
                    self.find_memory_allocations_from_store(target),
                    self.find_memory_allocations_from_expression(value),
                ])
            }
            Statement::IfStatement(IfStatement {
                condition: _,
                branches,
            }) => self.merge_memory_allocations(vec![
                self.find_memory_allocations_from_statements(&branches.0),
                self.find_memory_allocations_from_statements(&branches.1),
            ]),
            Statement::MatchStatement(MatchStatement {
                expression: _,
                branches,
            }) => self.merge_memory_allocations(
                branches
                    .iter()
                    .map(|branch| self.find_memory_allocations_from_statements(&branch.statements))
                    .collect_vec(),
            ),
        };
        allocations
    }
    fn find_memory_allocations_from_store(&self, store: &Store) -> Vec<MemoryAllocation> {
        match store {
            Store::Memory(id, machine_type) => {
                vec![MemoryAllocation(id.clone(), machine_type.clone())]
            }
            _ => Vec::new(),
        }
    }
    fn find_memory_allocations_from_expression(
        &self,
        expression: &Expression,
    ) -> Vec<MemoryAllocation> {
        match expression {
            Expression::Value(value) => self.find_memory_allocations_from_value(value),
            Expression::Wrap(value) => self.find_memory_allocations_from_value(value),
            Expression::TupleExpression(TupleExpression(expressions)) => {
                self.find_memory_allocations_from_values(expressions)
            }
            Expression::FnCall(FnCall { fn_, args }) => self.merge_memory_allocations(vec![
                self.find_memory_allocations_from_value(fn_),
                self.find_memory_allocations_from_values(args),
            ]),
            Expression::ConstructorCall(ConstructorCall { idx: _, data }) => {
                data.as_ref().map_or_else(Vec::new, |(_, value)| {
                    self.find_memory_allocations_from_value(&value)
                })
            }
            _ => Vec::new(),
        }
    }
    fn find_memory_allocations_from_values(&self, values: &Vec<Value>) -> Vec<MemoryAllocation> {
        values
            .iter()
            .map(|value| self.find_memory_allocations_from_value(value))
            .concat()
    }
    fn find_memory_allocations_from_value(&self, value: &Value) -> Vec<MemoryAllocation> {
        match value {
            Value::Block(Block { statements, ret: _ }) => {
                self.find_memory_allocations_from_statements(statements)
            }
            _ => Vec::new(),
        }
    }
    fn translate_memory_allocation(&self, memory_allocation: MemoryAllocation) -> Code {
        let MemoryAllocation(id, type_) = memory_allocation;
        format!("{} {id} = nullptr;", self.translate_type(&type_))
    }
    fn translate_memory_allocations(&self, memory_allocations: Vec<MemoryAllocation>) -> Code {
        memory_allocations
            .into_iter()
            .map(|memory_allocation| self.translate_memory_allocation(memory_allocation))
            .join("")
    }
    fn translate_fn_def(&self, fn_def: FnDef) -> Code {
        let name = fn_def.name;
        let return_type = fn_def.ret.type_();
        let MachineType::Lazy(raw_return_type) = &return_type else {
            panic!("Function has invalid return type.")
        };
        let raw_argument_types = fn_def
            .arguments
            .iter()
            .map(|(_, type_)| {
                let MachineType::Lazy(raw_argument_type) = &type_ else {
                    panic!("Function has invalid argument type.")
                };
                *raw_argument_type.clone()
            })
            .collect_vec();
        let base_name = "Closure";
        let memory_allocations = self.find_memory_allocations_from_statements(&fn_def.statements);
        let memory_allocations_code = self.translate_memory_allocations(memory_allocations);
        let statements_code = self.translate_statements(fn_def.statements);
        let return_code = format!("return {};", self.translate_store(fn_def.ret));
        let base = format!(
            "{base_name}<{name},{},{}>",
            fn_def
                .env
                .map_or_else(|| Code::from("Empty"), |type_| self.translate_type(&type_)),
            TypesFormatter(
                &std::iter::once(*raw_return_type.clone())
                    .chain(raw_argument_types.into_iter())
                    .collect_vec()
            )
        );
        let declaration = format!("struct {name} : {base}");
        let constructor_code = format!("using {base}::{base_name};");
        let header_code = format!(
            "{} body({}) override",
            self.translate_type(&return_type),
            fn_def
                .arguments
                .into_iter()
                .map(|(name, type_)| format!("{} &{name}", self.translate_type(&type_)))
                .join(",")
        );
        format!("{declaration} {{ {constructor_code} {memory_allocations_code} {header_code} {{ {statements_code} {return_code} }} }};")
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct MemoryAllocation(Id, MachineType);

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
                            .map(|type_| {
                                let MachineType::Lazy(t) = type_ else {
                                    panic!("Function type without lazy arguments and return.");
                                };
                                *t
                            })
                            .collect()
                    )
                )
            }
            MachineType::UnionType(UnionType(type_names)) => {
                write!(f, "VariantT<{}>", type_names.join(","))
            }
            MachineType::NamedType(name) => write!(f, "{}", name),
            MachineType::Reference(type_) => write!(f, "{}*", TypeFormatter(&**type_)),
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

    use lowering::{Block, Id, MatchBranch, Name};
    use once_cell::sync::Lazy;
    use regex::Regex;
    use test_case::test_case;

    const TRANSLATOR: Lazy<Translator> = Lazy::new(|| Translator {});

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
        FnType(Vec::new(), Box::new(MachineType::Lazy(Box::new(TupleType(Vec::new()).into())))).into(),
        "FnT<TupleT<>>";
        "unit fn type"
    )]
    #[test_case(
        FnType(
            vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),],
            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),)
        ).into(),
        "FnT<Int,Int>";
        "int identity fn"
    )]
    #[test_case(
        FnType(
            vec![
                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
            ],
            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into())),)
        ).into(),
        "FnT<Bool,Int,Int>";
        "int comparison fn"
    )]
    #[test_case(
        FnType(
            vec![
                MachineType::Lazy(Box::new(FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into())),)
                ).into())),
                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
            ],
            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into())),)
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
    #[test_case(
        MachineType::Reference(Box::new(
            MachineType::NamedType(
                Name::from("Cons")
            )
        )),
        "Cons*";
        "reference type"
    )]
    fn test_type_translation(type_: MachineType, expected: &str) {
        let code = TRANSLATOR.translate_type(&type_);
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
        "struct Twoo; struct Faws; typedef VariantT<Twoo, Faws> Bull; struct Twoo{}; struct Faws{};";
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
        "struct Left_IntBool; struct Right_IntBool; typedef VariantT<Left_IntBool, Right_IntBool> EitherIntBool; struct Left_IntBool { using type = Int; type value; }; struct Right_IntBool { using type = Bool; type value; };";
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
                        MachineType::Reference(Box::new(MachineType::NamedType(Name::from("ListInt"))))
                    ]).into())
                ),
                (Name::from("Nil_Int"), None)
            ]
        },
        "struct Cons_Int; struct Nil_Int; typedef VariantT<Cons_Int, Nil_Int> ListInt; struct Cons_Int{ using type = TupleT<Int,ListInt*>; type value;}; struct Nil_Int{};";
        "list int"
    )]
    fn test_typedef_translations(type_def: TypeDef, expected: &str) {
        let code = TRANSLATOR.translate_type_defs(vec![type_def]);
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
                                MachineType::Reference(Box::new(MachineType::NamedType(Name::from("Value")))),
                                MachineType::Reference(Box::new(MachineType::NamedType(Name::from("Value")))),
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
                        Some(MachineType::Reference(Box::new(MachineType::NamedType(Name::from("Expression")))))
                    ),
                ]
            }
        ],
        "struct Basic; struct Complex; struct None; struct Some; typedef VariantT<Basic,Complex> Expression; typedef VariantT<None,Some> Value; struct Basic { using type = Int; type value; }; struct Complex { using type = TupleT<Value*, Value*>; type value; }; struct None{}; struct Some { using type = Expression*; type value; };";
        "mutually recursive types"
    )]
    fn test_typedefs_translations(type_defs: Vec<TypeDef>, expected: &str) {
        let code = TRANSLATOR.translate_type_defs(type_defs);
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
        let code = TRANSLATOR.translate_builtin(value);
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
    #[test_case(
        Store::Global(Id::from("baz"), AtomicType(AtomicTypeEnum::BOOL).into()),
        "baz";
        "global translation"
    )]
    fn test_store_translation(store: Store, expected: &str) {
        let code = TRANSLATOR.translate_store(store);
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
        let code = TRANSLATOR.translate_value(value);
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
        let code = TRANSLATOR.translate_expression(expression);
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
            target: Store::Global(Id::from("x"), AtomicType(AtomicTypeEnum::INT).into()).into(),
            value: Value::BuiltIn(Integer{value: -5}.into()).into()
        },
        "x = -5LL;";
        "global integer assignment"
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
            target: Store::Register(Id::from("y"), AtomicType(AtomicTypeEnum::BOOL).into()).into(),
            value: Value::BuiltIn(Boolean{value: true}.into()).into(),
        },
        "Bool y = true;";
        "boolean assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("y"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
            value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into())),
        },
        "Lazy<Bool>* y = new LazyConstant<Bool>{true};";
        "wrapping constant"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(
                Id::from("g"),
                MachineType::Lazy(
                    Box::new(
                        FnType(
                            vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),),
                        ).into()
                    )
                )
            ),
            value: Expression::Wrap(Store::Register(
                Id::from("f"),
                FnType(
                    vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),),
                ).into()
            ).into()),
        },
        "if (g == nullptr) { g = new LazyConstant<FnT<Int,Int>>{f}; }";
        "wrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(
                Id::from("w"),
                FnType(
                    vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                ).into()
            ).into(),
            value: Expression::Unwrap(
                Store::Memory(
                    Id::from("g"),
                    MachineType::Lazy(
                        Box::new(
                            FnType(
                                vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        )
                    )
                )
            ),
        },
        "if (w == nullptr) { w = g->value(); }";
        "unwrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("y"), AtomicType(AtomicTypeEnum::BOOL).into()).into(),
            value: Expression::Unwrap(
                Store::Memory(
                    Id::from("t"),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))
                )
            ),
        },
        "Bool y = t->value();";
        "unwrapping boolean from variable"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            value: FnCall{
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Plus__BuiltIn"),
                    FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                ).into(),
                args: vec![
                    Store::Register(Id::from("arg1"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                    Store::Register(Id::from("arg2"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                ]
            }.into()
        },
        "if (call == nullptr) { call = new Plus__BuiltIn{}; call->args = std::make_tuple(arg1, arg2); call->call(); }";
        "built-in fn-call"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            value: FnCall{
                fn_: Block{
                    statements: Vec::new(),
                    ret: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))
                }.into(),
                args: Vec::new()
            }.into()
        },
        "if (call == nullptr) { call = new BlockFn<Int>([&](){ return call; }); call->args = std::make_tuple(); call->call(); }";
        "empty block fn-call"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("block"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            value: FnCall{
                fn_: Block{
                    statements: vec![
                        Assignment {
                            target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Increment__BuiltIn"),
                                    FnType(
                                        vec![
                                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                        ],
                                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                    ).into()
                                ).into(),
                                args: vec![
                                    Store::Memory(
                                        Id::from("x"),
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                    ).into()
                                ]
                            }.into()
                        }.into(),
                    ],
                    ret: Store::Memory(
                        Id::from("call"),
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                    )
                }.into(),
                args: Vec::new()
            }.into()
        },
        "if (block == nullptr) { block = new BlockFn<Int>([&]() { if (call == nullptr) { call = new Increment__BuiltIn{}; call->args = std::make_tuple(x); call->call(); } return call; }); block->args = std::make_tuple(); block->call();}";
        "internal fn-call block fn-call"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(
                Id::from("block"),
                FnType(
                    Vec::new(),
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                ).into()
            ),
            value: Expression::Value(Block{
                statements: vec![
                    Assignment {
                        target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Decrement__BuiltIn"),
                                FnType(
                                    vec![
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    ],
                                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                ).into()
                            ).into(),
                            args: vec![
                                Store::Memory(
                                    Id::from("y"),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                ).into()
                            ]
                        }.into()
                    }.into(),
                ],
                ret: Store::Memory(
                    Id::from("call"),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                )
            }.into())
        },
        "FnT<Int> block = new BlockFn<Int>([&](){ if (call == nullptr) { call = new Decrement__BuiltIn{}; call->args = std::make_tuple(y); call->call(); } return call; });";
        "block assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(Id::from("call2"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            value: FnCall{
                fn_: Store::Memory(
                    Name::from("call1"),
                    FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                ).into(),
                args: vec![
                    Store::Register(Id::from("arg1"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                    Store::Register(Id::from("arg2"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                ]
            }.into()
        },
        "if (call2 == nullptr) { call2 = call1->clone();  call2->args = std::make_tuple(arg1, arg2); call2->call(); }";
        "custom fn-call"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("e"), TupleType(Vec::new()).into()),
            value: TupleExpression(Vec::new()).into()
        },
        "TupleT<> e = std::make_tuple();";
        "empty tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("t"), TupleType(vec![AtomicType(AtomicTypeEnum::INT).into()]).into()),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: 5}.into())
            ]).into()
        },
        "TupleT<Int> t = std::make_tuple(5LL);";
        "singleton tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("t"), TupleType(vec![AtomicType(AtomicTypeEnum::INT).into(),AtomicType(AtomicTypeEnum::INT).into()]).into()),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: -4}.into()),
                Store::Register(Id::from("y"), AtomicType(AtomicTypeEnum::INT).into()).into()
            ]).into()
        },
        "TupleT<Int,Int> t = std::make_tuple(-4LL,y);";
        "double tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(Id::from("bull"), MachineType::NamedType(Name::from("Bull"))),
            value: ConstructorCall {
                idx: 1,
                data: None
            }.into()
        },
        "Bull bull = {}; bull.tag = 1ULL;";
        "empty constructor assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(
                Id::from("wrapper"),
                UnionType(vec![Name::from("Wrapper")]).into()
            ),
            value: ConstructorCall {
                idx: 0,
                data: Some((Name::from("Wrapper"), Value::BuiltIn(Integer{value: 4}.into())))
            }.into()
        },
        "VariantT<Wrapper> wrapper = {}; reinterpret_cast<Wrapper*>(&wrapper.value)->value = 4LL; wrapper.tag = 0ULL;";
        "wrapper constructor assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Register(
                Id::from("lr"),
                MachineType::Reference(Box::new(MachineType::NamedType(Name::from("ListInt"))))
            ).into(),
            value: Expression::Reference(
                Store::Register(
                    Id::from("l"),
                    MachineType::NamedType(Name::from("ListInt"))
                ).into()
            )
        },
        "ListInt* lr = new ListInt{l};";
        "reference assignment"
    )]
    #[test_case(
        Assignment {
            target: Store::Memory(
                Id::from("closure"),
                FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                ).into()
            ).into(),
            value: ClosureInstantiation{
                name: Name::from("Adder"),
                env: Store::Register(
                    Id::from("x"),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                ).into()
            }.into()
        },
        "if (closure == nullptr) { closure = new Adder{x}; }";
        "closure assignment"
    )]
    fn test_assignment_translation(assignment: Assignment, expected: &str) {
        let code = TRANSLATOR.translate_assignment(assignment);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        MatchStatement{
            expression: Store::Register(
                Id::from("bull"),
                UnionType(vec![Name::from("Twoo"), Name::from("Faws")]).into()
            ),
            branches: vec![
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                            value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into()).into())
                        }.into(),
                    ],
                },
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                            value: Expression::Wrap(Value::BuiltIn(Boolean{value: false}.into()).into())
                        }.into(),
                    ],
                }
            ]
        },
        "switch (bull.tag) { case 0ULL: { if (r == nullptr) { r = new LazyConstant<Bool>{true}; } break; } case 1ULL: { if (r == nullptr) { r = new LazyConstant<Bool>{false}; } break; } }";
        "match statement no values"
    )]
    #[test_case(
        MatchStatement {
            expression: Store::Register(
                Id::from("either"),
                UnionType(vec![Name::from("Left"), Name::from("Right")]).into()
            ),
            branches: vec![
                MatchBranch {
                    target: Some(Name::from("x")),
                    statements: vec![
                        Assignment {
                            target: Store::Register(Id::from("z"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                            value: Expression::Wrap(Store::Register(Name::from("x"), AtomicType(AtomicTypeEnum::INT).into()).into())
                        }.into(),
                        Assignment {
                            target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Comparison_GE__BuiltIn"),
                                    FnType(
                                        vec![
                                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                        ],
                                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))),
                                    ).into()
                                ).into(),
                                args: vec![
                                    Store::Register(Id::from("z"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                                    Store::Register(Id::from("y"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                                ]
                            }.into()
                        }.into()
                    ],
                },
                MatchBranch {
                    target: Some(Name::from("x")),
                    statements: vec![
                        Assignment {
                            target: Store::Memory(Id::from("call"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                            value: Expression::Wrap(Store::Register(Name::from("x"), AtomicType(AtomicTypeEnum::BOOL).into()).into())
                        }.into(),
                    ],
                }
            ]
        },
        "switch (either.tag) { case 0ULL: { Left::type x = reinterpret_cast<Left*>(&either.value)->value; Lazy<Int>* z = new LazyConstant<Int>{x}; if (call == nullptr) { call = new Comparison_GE__BuiltIn{}; call->args = std::make_tuple(z, y); call->call(); } break; } case 1ULL: { Right::type x = reinterpret_cast<Right*>(&either.value)->value; if (call == nullptr) { call = new LazyConstant<Bool>{x};} break; }}";
        "match statement read values"
    )]
    #[test_case(
        MatchStatement {
            expression: Store::Register(
                Id::from("nat"),
                UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()
            ),
            branches: vec![
                MatchBranch {
                    target: Some(Name::from("s")),
                    statements: vec![
                        Assignment {
                            target: Store::Register(Id::from("u"), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()).into(),
                            value: Expression::Dereference(Store::Register(Name::from("s"), MachineType::NamedType(Name::from("Suc")).into()).into())
                        }.into(),
                        Assignment {
                            target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))).into(),
                            value: Expression::Wrap(Store::Register(Name::from("u"), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()).into())
                        }.into(),
                    ],
                },
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))).into(),
                            value: Expression::Wrap(Store::Register(Name::from("nil"), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()).into())
                        }.into(),
                    ],
                }
            ]
        },
        "switch (nat.tag) { case 0ULL: { Suc::type s = reinterpret_cast<Suc*>(&nat.value)->value; VariantT<Suc,Nil> u = *s; if (r == nullptr) { r = new LazyConstant<VariantT<Suc,Nil>>{u};} break; } case 1ULL: { if (r == nullptr) {r = new LazyConstant<VariantT<Suc,Nil>>{nil};} break; }}";
        "match statement recursive type"
    )]
    fn test_match_statement_translation(match_statement: MatchStatement, expected: &str) {
        let code = TRANSLATOR.translate_match_statement(match_statement);
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
    #[test_case(
        IfStatement {
            condition: Store::Register(Id::from("z"), AtomicType(AtomicTypeEnum::BOOL).into()),
            branches: (
                vec![Assignment {
                    target: Store::Memory(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                    value: Expression::Wrap(Value::BuiltIn(Integer{value: 1}.into()).into())
                }.into()],
                vec![Assignment {
                    target: Store::Memory(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                    value: Expression::Wrap(Value::BuiltIn(Integer{value: -1}.into()).into())
                }.into()],
            )
        }.into(),
        "if (z) { if (x == nullptr) { x = new LazyConstant<Int>{1LL}; } } else { if (x == nullptr) { x = new LazyConstant<Int>{-1LL}; } }";
        "if-else statement"
    )]
    #[test_case(
        IfStatement {
            condition: Store::Register(Id::from("z"), AtomicType(AtomicTypeEnum::BOOL).into()),
            branches: (
                vec![
                    IfStatement {
                        condition: Store::Register(Id::from("y"), AtomicType(AtomicTypeEnum::BOOL).into()),
                        branches: (
                            vec![Assignment {
                                target: Store::Memory(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                                value: Expression::Wrap(Value::BuiltIn(Integer{value: 1}.into()).into())
                            }.into()],
                            vec![Assignment {
                                target: Store::Memory(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                                value: Expression::Wrap(Value::BuiltIn(Integer{value: -1}.into()).into())
                            }.into()],
                        )
                    }.into(),
                    Assignment {
                        target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                        value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into()).into())
                    }.into(),
                ],
                vec![
                    Assignment {
                        target: Store::Memory(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                        value: Expression::Wrap(Value::BuiltIn(Integer{value: 0}.into()).into())
                    }.into(),
                    Assignment {
                        target: Store::Memory(Id::from("r"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))).into(),
                        value: Expression::Wrap(Value::BuiltIn(Boolean{value: false}.into()).into())
                    }.into(),
                ],
            )
        }.into(),
        "if (z) { if (y) { if (x == nullptr){ x = new LazyConstant<Int>{1LL}; } } else {if (x == nullptr){ x = new LazyConstant<Int>{-1LL}; } } if (r == nullptr) { r = new LazyConstant<Bool>{true}; } } else { if (x == nullptr){ x = new LazyConstant<Int>{0LL}; } if (r == nullptr) { r = new LazyConstant<Bool>{false}; } }";
        "nested if-else statement"
    )]
    fn test_statement_translation(statement: Statement, expected: &str) {
        let code = TRANSLATOR.translate_statement(statement);
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
    #[test_case(
        vec![
            Assignment {
                target: Store::Register(Name::from("tail_"), MachineType::Reference(Box::new(MachineType::NamedType(Name::from("List"))))).into(),
                value: ElementAccess{
                    value: Store::Register(Name::from("cons"), MachineType::NamedType(Name::from("Cons")).into()).into(),
                    idx: 1
                }.into()
            }.into(),
            Assignment {
                target: Store::Register(Id::from("tail"), UnionType(vec![Name::from("Cons"), Name::from("Nil")]).into()).into(),
                value: Expression::Dereference(Store::Register(Name::from("tail_"), MachineType::NamedType(Name::from("List")).into()).into())
            }.into(),
        ],
        "List *tail_ = std::get<1ULL>(cons); VariantT<Cons,Nil> tail = *tail_;";
        "cons extraction"
    )]
    #[test_case(
        vec![
            Assignment {
                target: Store::Register(
                    Name::from("n"),
                    UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()
                ).into(),
                value: ConstructorCall{
                    idx: 1,
                    data: None
                }.into()
            }.into(),
            Assignment {
                target: Store::Register(
                    Id::from("wrapped_n"),
                    MachineType::Reference(Box::new(UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))
                ).into(),
                value: Expression::Reference(Store::Register(Name::from("n"), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))
            }.into(),
            Assignment {
                target: Store::Register(Name::from("s"), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()).into(),
                value: ConstructorCall{
                    idx: 0,
                    data: Some((
                        Name::from("Suc"),
                        Store::Register(
                            Id::from("wrapped_n"),
                            MachineType::Reference(Box::new(UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))
                        ).into()
                    ))
                }.into()
            }.into(),
        ],
        "VariantT<Suc, Nil> n = {}; n.tag = 1ULL; VariantT<Suc, Nil> *wrapped_n = new VariantT<Suc, Nil>{n}; VariantT<Suc, Nil> s = {}; reinterpret_cast<Suc *>(&s.value)->value = wrapped_n; s.tag = 0ULL;";
        "simple recursive type extraction"
    )]
    fn test_statements_translation(statements: Vec<Statement>, expected: &str) {
        let code = TRANSLATOR.translate_statements(statements);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        FnDef {
            env: None,
            name: Name::from("IdentityInt"),
            arguments: vec![(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))],
            statements: Vec::new(),
            ret: Store::Register(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
        }
        ,
        "struct IdentityInt : Closure<IdentityInt, Empty, Int, Int> { using Closure<IdentityInt, Empty, Int, Int>::Closure; Lazy<Int> *body(Lazy<Int> *&x) override { return x; } };";
        "identity int"
    )]
    #[test_case(
        FnDef {
            env: None,
            name: Name::from("FourWayPlus"),
            arguments: vec![
                (Id::from("a"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Id::from("b"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Id::from("c"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Id::from("d"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            ],
            statements: vec![
                Assignment {
                    target: Store::Memory(
                        Id::from("call1"),
                            FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ).into()
                    ),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                            FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        ).into(),
                        args: vec![
                            Store::Register(Id::from("a"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                            Store::Register(Id::from("b"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Store::Memory(
                        Id::from("call2"),
                            FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ).into()
                    ),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                            FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        ).into(),
                        args: vec![
                            Store::Register(Id::from("c"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                            Store::Register(Id::from("d"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Store::Memory(
                        Id::from("call3"),
                            FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ).into()
                    ),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                            FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        ).into(),
                        args: vec![
                            Store::Register(Id::from("call1"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                            Store::Register(Id::from("call2"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                        ]
                    }.into()
                }.into(),
            ],
            ret: Store::Memory(Id::from("call3"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
        },
        "struct FourWayPlus : Closure<FourWayPlus, Empty, Int, Int, Int, Int, Int> { using Closure<FourWayPlus, Empty, Int, Int, Int, Int, Int>::Closure; FnT<Int,Int,Int> call1 = nullptr; FnT<Int,Int,Int> call2 = nullptr; FnT<Int,Int,Int> call3 = nullptr; Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c, Lazy<Int> *&d) override { if (call1 == nullptr) { call1 = new Plus__BuiltIn{}; call1->args = std::make_tuple(a, b); call1->call(); } if (call2 == nullptr) { call2 = new Plus__BuiltIn{}; call2->args = std::make_tuple(c, d); call2->call(); } if (call3 == nullptr) { call3 = new Plus__BuiltIn{}; call3->args = std::make_tuple(call1, call2); call3->call(); } return call3; } };";
        "four way plus int"
    )]
    #[test_case(
        FnDef {
            env: None,
            name: Name::from("FlatBlockExample"),
            arguments: vec![
                (Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            ],
            statements: vec![
                Assignment {
                    target: Store::Memory(
                        Id::from("block"),
                        FnType(
                            Vec::new(),
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ).into()
                    ),
                    value: FnCall{
                        fn_: Block{
                            statements: vec![
                                Assignment {
                                    target: Store::Memory(
                                        Id::from("call"),
                                        FnType(
                                            vec![
                                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                            ],
                                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                        ).into()
                                    ),
                                    value: FnCall{
                                        fn_: BuiltIn::BuiltInFn(
                                            Name::from("Increment__BuiltIn"),
                                            FnType(
                                                vec![
                                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                                ],
                                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                            ).into()
                                        ).into(),
                                        args: vec![
                                            Store::Memory(
                                                Id::from("x"),
                                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                            ).into()
                                        ]
                                    }.into()
                                }.into(),
                            ],
                            ret: Store::Memory(
                                Id::from("call"),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            )
                        }.into(),
                        args: Vec::new()
                    }.into()
                }.into(),
            ],
            ret: Store::Memory(
                Id::from("block"),
                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
            ),
        },
        "struct FlatBlockExample : Closure<FlatBlockExample, Empty, Int, Int> { using Closure<FlatBlockExample, Empty, Int, Int>::Closure; FnT<Int> block = nullptr; FnT<Int,Int> call = nullptr; Lazy<Int> *body(Lazy<Int> *&x) override { if (block == nullptr) { block = new BlockFn<Int>([&]() { if (call == nullptr) { call = new Increment__BuiltIn{}; call->args = std::make_tuple(x); call->call(); } return call; }); block->args = std::make_tuple(); block->call(); } return block; } }; ";
        "flat block example"
    )]
    #[test_case(
        FnDef{
            env: Some(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            name: Name::from("Adder"),
            arguments: vec![(Name::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))],
            statements: vec![
                Assignment {
                    target: Store::Memory(
                        Id::from("inner_res"),
                        FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ).into()
                    ).into(),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                            FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        ).into(),
                        args: vec![
                            Store::Register(Id::from("x"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                            Store::Memory(Id::from("env"), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))).into(),
                        ]
                    }.into()
                }.into()
            ],
            ret: Store::Memory(
                Id::from("inner_res"),
                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
            ).into()
        },
    "struct Adder : Closure<Adder, Lazy<Int> *, Int, Int> { using Closure<Adder, Lazy<Int> *, Int, Int>::Closure; FnT<Int, Int, Int> inner_res = nullptr; Lazy<Int> *body(Lazy<Int> *&x) override { if (inner_res == nullptr) { inner_res = new Plus__BuiltIn{}; inner_res->args = std::make_tuple(x, env); inner_res->call(); } return inner_res; } };";
    "adder closure"
    )]
    fn test_fn_def_translation(fn_def: FnDef, expected: &str) {
        let code = TRANSLATOR.translate_fn_def(fn_def);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }
}
