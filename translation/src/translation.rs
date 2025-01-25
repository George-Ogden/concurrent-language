use core::fmt;
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    fmt::Formatter,
};

use compilation::{
    Assignment, AtomicType, AtomicTypeEnum, Await, Block, Boolean, BuiltIn, ClosureInstantiation,
    ConstructorCall, Declaration, ElementAccess, Expression, FnCall, FnDef, FnType, Id,
    IfStatement, Integer, MachineType, MatchStatement, Memory, Name, Program, Statement,
    TupleExpression, TupleType, TypeDef, UnionType, Value,
};

type Code = String;

pub struct Translator {}

impl Translator {
    fn translate_type(&self, type_: &MachineType) -> Code {
        format!("{}", TypeFormatter(type_))
    }
    fn top_sort(&self, type_defs: &Vec<TypeDef>) -> Vec<(Name, Option<MachineType>)> {
        let mut visited = HashSet::<Name>::new();
        let mut result = Vec::new();
        let type_defs_by_name = type_defs
            .iter()
            .map(|type_def| (type_def.name.clone(), type_def.clone()))
            .collect();
        for type_def in type_defs {
            self.top_sort_internal(
                type_def.clone(),
                &type_defs_by_name,
                &mut visited,
                &mut result,
            );
        }
        result
    }
    fn top_sort_internal(
        &self,
        type_def: TypeDef,
        type_defs: &HashMap<Name, TypeDef>,
        visited: &mut HashSet<Name>,
        result: &mut Vec<(Name, Option<MachineType>)>,
    ) {
        if visited.contains(&type_def.name) {
            return;
        }
        visited.insert(type_def.name.clone());
        let used_types = type_def.directly_used_types();
        for type_name in used_types {
            let type_name = type_name
                .split_once("C")
                .map_or(type_name.clone(), |(before, _)| String::from(before));
            self.top_sort_internal(type_defs[&type_name].clone(), type_defs, visited, result);
        }
        result.extend(type_def.constructors.iter().map(|ctor| ctor.clone()));
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
        let ctors = self.top_sort(&type_defs);
        let constructor_definitions = ctors.iter().map(|(name, type_)| {
            let fields = match type_ {
                Some(type_) => {
                    format!("using type = {}; type value;", self.translate_type(type_))
                }
                None => Code::from("Empty value;"),
            };
            format!("struct {name} {{ {fields} }};")
        });
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
            BuiltIn::BuiltInFn(name) => format!("new {name}{{}}"),
        }
    }
    fn translate_memory(&self, memory: Memory) -> Code {
        memory.0
    }
    fn translate_block(&self, block: Block) -> Code {
        let statements_code = self.translate_statements(block.statements);
        let MachineType::Lazy(type_) = &block.ret.1 else {
            panic!("Block has non-lazy return type.")
        };
        let type_code = self.translate_type(&*type_);
        let return_code = format!("return {};", self.translate_value(block.ret.0));
        format!("new BlockFn<{type_code}>([&]() {{ {statements_code} {return_code} }})")
    }
    fn translate_value(&self, value: Value) -> Code {
        match value {
            Value::BuiltIn(value) => self.translate_builtin(value),
            Value::Memory(memory) => self.translate_memory(memory),
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
                format!("std::get<{idx}ULL>({})", self.translate_value(value))
            }
            Expression::Value(value) => self.translate_value(value),
            Expression::Wrap(value, type_) => format!(
                "new LazyConstant<{}>{{{}}}",
                self.translate_type(&type_),
                self.translate_value(value)
            ),
            Expression::Unwrap(value) => format!("{}->value()", self.translate_value(value)),
            Expression::TupleExpression(TupleExpression(values)) => {
                format!("std::make_tuple({})", self.translate_value_list(values))
            }
            Expression::Block(block) => self.translate_block(block),
            e => panic!("{:?} does not translate directly as an expression", e),
        }
    }
    fn translate_await(&self, await_: Await) -> Code {
        let arguments = await_
            .0
            .into_iter()
            .map(|memory| self.translate_memory(memory))
            .join(",");
        format!("WorkManager::await({arguments});")
    }
    fn translate_fn_call(&self, target: Id, fn_call: FnCall) -> Code {
        let fn_initialization_code = match fn_call.fn_ {
            Value::BuiltIn(built_in) => {
                format!("{};", self.translate_builtin(built_in))
            }
            Value::Memory(memory) => {
                let memory_code = self.translate_memory(memory);
                format!("{memory_code}->clone();",)
            }
        };
        let type_code = self.translate_type(&fn_call.fn_type.into());
        let args_assignment = format!(
            "dynamic_cast<{type_code}>({target})->args = std::make_tuple({});",
            self.translate_value_list(fn_call.args)
        );
        format!("{fn_initialization_code} {args_assignment} dynamic_cast<{type_code}>({target})->call()",)
    }
    fn translate_constructor_call(&self, target: Id, constructor_call: ConstructorCall) -> Code {
        let declaration = format!("{{}};");
        let indexing_code = format!("{target}.tag = {}ULL", constructor_call.idx);
        let value_code = match constructor_call.data {
            None => Code::new(),
            Some((name, value)) => format!(
                "reinterpret_cast<{name}*>(&{target}.value)->value = create_references<{name}::type>({});",
                self.translate_value(value)
            ),
        };
        format!("{declaration} {value_code} {indexing_code}")
    }
    fn translate_declaration(&self, declaration: Declaration) -> Code {
        let Declaration { type_, memory } = declaration;
        format!(
            "{} {};",
            self.translate_type(&type_),
            self.translate_memory(memory)
        )
    }
    fn translate_assignment(&self, assignment: Assignment) -> Code {
        let Memory(id) = assignment.target;
        let value_code = match assignment.value {
            Expression::FnCall(fn_call) => self.translate_fn_call(id.clone(), fn_call),
            Expression::ConstructorCall(constructor_call) => {
                self.translate_constructor_call(id.clone(), constructor_call)
            }
            Expression::ClosureInstantiation(ClosureInstantiation { name, env }) => {
                return env.map_or_else(Code::new, |env| {
                    format!(
                        "dynamic_cast<{name}*>({id})->env = {};",
                        self.translate_value(env)
                    )
                })
            }
            value => self.translate_expression(value),
        };
        let assignment_code = format!("{id} = {value_code};");
        if assignment.check_null {
            format!("if ({id} == nullptr) {{ {assignment_code} }}")
        } else {
            assignment_code
        }
    }
    fn translate_if_statement(&self, if_statement: IfStatement) -> Code {
        let condition_code = self.translate_value(if_statement.condition);
        let if_branch = self.translate_statements(if_statement.branches.0);
        let else_branch = self.translate_statements(if_statement.branches.1);
        format!("if ({condition_code}) {{ {if_branch} }} else {{ {else_branch} }}",)
    }
    fn translate_match_statement(&self, match_statement: MatchStatement) -> Code {
        let UnionType(types) = match_statement.expression.1;
        let expression_code = self.translate_value(match_statement.expression.0);
        let branches_code = match_statement
            .branches
            .into_iter()
            .enumerate()
            .map(|(i, branch)| {
                let assignment_code = match branch.target {
                    Some(Memory(id)) => {
                        let type_name = &types[i];
                        let lazy_type = format!("destroy_references_t<{type_name}::type>");
                        format!(
                            "Lazy<{lazy_type}> *{id} = new LazyConstant<{lazy_type}>{{destroy_references(reinterpret_cast<{type_name}*>(&{expression_code}.value)->value)}};",
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
        format!("switch ({expression_code}.tag) {{ {branches_code} }}")
    }
    fn translate_statement(&self, statement: Statement) -> Code {
        match statement {
            Statement::Await(await_) => self.translate_await(await_),
            Statement::Assignment(assignment) => self.translate_assignment(assignment),
            Statement::IfStatement(if_statement) => self.translate_if_statement(if_statement),
            Statement::Declaration(declaration) => self.translate_declaration(declaration),
            Statement::MatchStatement(match_statement) => {
                self.translate_match_statement(match_statement)
            }
        }
    }
    fn translate_statements(&self, statements: Vec<Statement>) -> Code {
        let (declarations, other_statements): (Vec<_>, Vec<_>) = statements
            .into_iter()
            .partition(|statement| matches!(statement, Statement::Declaration(_)));

        let declarations_code = declarations
            .into_iter()
            .map(|statement| self.translate_statement(statement))
            .join("\n");

        let closure_predefinitions = other_statements
            .iter()
            .filter_map(|statement| {
                if let Statement::Assignment(Assignment {
                    target,
                    value: Expression::ClosureInstantiation(ClosureInstantiation { name, env: _ }),
                    check_null,
                }) = statement
                {
                    if *check_null {
                        let target = self.translate_memory(target.clone());
                        Some(format!(
                            "if ({target} == nullptr) {{ {target} = new {name}{{}}; }}"
                        ))
                    } else {
                        Some(format!(
                            "{} = new {name}{{}};",
                            self.translate_memory(target.clone())
                        ))
                    }
                } else {
                    None
                }
            })
            .join("\n");

        let other_code = other_statements
            .into_iter()
            .map(|statement| self.translate_statement(statement))
            .join("\n");

        format!("{declarations_code}\n{closure_predefinitions}\n{other_code}")
    }
    fn translate_memory_allocation(&self, memory_allocation: Declaration) -> Code {
        let Declaration { memory, type_ } = memory_allocation;
        format!(
            "{} {} = nullptr;",
            self.translate_type(&type_),
            self.translate_memory(memory)
        )
    }
    fn translate_memory_allocations(&self, memory_allocations: Vec<Declaration>) -> Code {
        memory_allocations
            .into_iter()
            .map(|memory_allocation| self.translate_memory_allocation(memory_allocation))
            .join("\n")
    }
    fn translate_fn_def(&self, fn_def: FnDef) -> Code {
        let name = fn_def.name;
        let return_type = fn_def.ret.1;
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
        let memory_allocations = fn_def.allocations;
        let memory_allocations_code = self.translate_memory_allocations(memory_allocations);
        let statements_code = self.translate_statements(fn_def.statements);
        let return_code = format!("return {};", self.translate_value(fn_def.ret.0));
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
                .map(|(memory, type_)| format!(
                    "{} &{}",
                    self.translate_type(&type_),
                    self.translate_memory(memory)
                ))
                .join(",")
        );
        format!("{declaration} {{ {constructor_code} {memory_allocations_code} {header_code} {{ {statements_code} {return_code} }} }};")
    }
    fn translate_fn_defs(&self, fn_defs: Vec<FnDef>) -> Code {
        fn_defs
            .into_iter()
            .map(|fn_def| self.translate_fn_def(fn_def))
            .join("\n")
    }
    fn translate_program(&self, program: Program) -> Code {
        let type_def_code = self.translate_type_defs(program.type_defs);
        let fn_def_code = self.translate_fn_defs(program.fn_defs);
        format!("#include \"main/include.hpp\"\n\n{type_def_code} {fn_def_code}")
    }
    pub fn translate(program: Program) -> Code {
        let translator = Translator {};
        translator.translate_program(program)
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

    use compilation::{Block, Id, MatchBranch, Name};
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
        "struct Twoo; struct Faws; typedef VariantT<Twoo, Faws> Bull; struct Twoo{ Empty value; }; struct Faws{ Empty value; };";
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
        "struct Cons_Int; struct Nil_Int; typedef VariantT<Cons_Int, Nil_Int> ListInt; struct Cons_Int{ using type = TupleT<Int,ListInt*>; type value;}; struct Nil_Int{ Empty value; };";
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
        "struct Basic; struct Complex; struct None; struct Some; typedef VariantT<Basic,Complex> Expression; typedef VariantT<None,Some> Value; struct Basic { using type = Int; type value; }; struct Complex { using type = TupleT<Value*, Value*>; type value; }; struct None{Empty value;}; struct Some { using type = Expression*; type value; };";
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
        ),
        "new Plus__BuiltIn{}";
        "builtin plus translation"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_GE__BuiltIn"),
        ),
        "new Comparison_GE__BuiltIn{}";
        "builtin greater than or equal to translation"
    )]
    fn test_builtin_translation(value: BuiltIn, expected: &str) {
        let code = TRANSLATOR.translate_builtin(value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(Memory(Id::from("x")), "x")]
    #[test_case(Memory(Id::from("bar")), "bar")]
    #[test_case(Memory(Id::from("baz")), "baz")]
    fn test_memory_translation(memory: Memory, expected: &str) {
        let code = TRANSLATOR.translate_memory(memory);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Memory(Id::from("baz")).into(),
        "baz";
        "value memory translation"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_LT__BuiltIn"),
        ).into(),
        "new Comparison_LT__BuiltIn{}";
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
            value: Memory(
                Name::from("tuple"),
            ).into(),
            idx: 1
        }.into(),
        "std::get<1ULL>(tuple)";
        "tuple index access"
    )]
    #[test_case(
        Expression::Wrap(
            BuiltIn::BuiltInFn(Name::from("Plus__BuiltIn")).into(),
            FnType(
                vec![
                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                ],
                Box::new(
                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                )
            ).into()
        ),
        "new LazyConstant<FnT<Int,Int,Int>>{new Plus__BuiltIn{}}";
        "lazy function instantiation"
    )]
    fn test_expression_translation(expression: Expression, expected: &str) {
        let code = TRANSLATOR.translate_expression(expression);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Assignment {
            target: Memory(Id::from("x")).into(),
            value: Value::BuiltIn(Integer{value: 5}.into()).into(),
            check_null: false
        },
        "x = 5LL;";
        "integer assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("x"), ),
            value: ElementAccess{
                value: Value::Memory(Memory(
                    Name::from("tuple")
                ).into()),
                idx: 0
            }.into(),
            check_null: false
        },
        "x = std::get<0ULL>(tuple);";
        "tuple access assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("y")).into(),
            value: Value::BuiltIn(Boolean{value: true}.into()).into(),
            check_null: false
        },
        "y = true;";
        "boolean assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("x")).into(),
            value: Expression::Wrap(Value::BuiltIn(Integer{value: -5}.into()), AtomicType(AtomicTypeEnum::INT).into()),
            check_null: true
        },
        "if (x == nullptr) { x = new LazyConstant<Int>{-5LL}; }";
        "wrapping constant"
    )]
    #[test_case(
        Assignment {
            target: Memory(
                Id::from("g"),
            ),
            value: Expression::Wrap(
                Memory(
                    Id::from("f"),
                ).into(),
                FnType(
                    vec![MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),),
                ).into()
            ),
            check_null: true
        },
        "if (g == nullptr) { g = new LazyConstant<FnT<Int,Int>>{f}; }";
        "wrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("w")).into(),
            value: Expression::Unwrap(Memory(Id::from("g")).into()),
            check_null: false
        },
        "w = g->value();";
        "unwrapping function from variable"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("y")).into(),
            value: Expression::Unwrap(Memory(Id::from("t")).into()),
            check_null: false
        },
        "y = t->value();";
        "unwrapping boolean from variable"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("call")),
            value: FnCall{
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Plus__BuiltIn"),
                ).into(),
                fn_type: FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                ),
                args: vec![
                    Memory(Id::from("arg1")).into(),
                    Memory(Id::from("arg2")).into(),
                ]
            }.into(),
            check_null: true
        },
        "if (call == nullptr) { call = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(call)->args = std::make_tuple(arg1, arg2); dynamic_cast<FnT<Int,Int,Int>>(call)->call(); }";
        "built-in fn call"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("call")),
            value: Block{
                statements: Vec::new(),
                ret: (Memory(Id::from("call")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))
            }.into(),
            check_null: false
        },
        "call = new BlockFn<Int>([&](){ return call; });";
        "empty block"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("block")),
            value: Block{
                statements: vec![
                    Assignment {
                        target: Memory(Id::from("call")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Increment__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ),
                            args: vec![
                                Memory(
                                    Id::from("x")
                                ).into()
                            ]
                        }.into(),
                        check_null: true
                    }.into(),
                ],
                ret: (
                    Memory(
                        Id::from("call"),
                    ).into(),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                )
            }.into(),
            check_null: true
        },
        "if (block == nullptr) { block = new BlockFn<Int>([&]() { if (call == nullptr) { call = new Increment__BuiltIn{}; dynamic_cast<FnT<Int,Int>>(call)->args = std::make_tuple(x); dynamic_cast<FnT<Int,Int>>(call)->call(); } return call; });}";
        "internal fn call block fn call"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("block")),
            value: Block{
                statements: vec![
                    Assignment {
                        target: Memory(Id::from("call")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Decrement__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ),
                            args: vec![Memory(Id::from("y")).into()]
                        }.into(),
                        check_null: true
                    }.into(),
                ],
                ret: (
                    Memory(Id::from("call")).into(),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                )
            }.into(),
            check_null: false
        },
        "block = new BlockFn<Int>([&](){ if (call == nullptr) { call = new Decrement__BuiltIn{}; dynamic_cast<FnT<Int,Int>>(call)->args = std::make_tuple(y); dynamic_cast<FnT<Int,Int>>(call)->call(); } return call; });";
        "block assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("call2")),
            value: FnCall{
                fn_: Memory(Name::from("call1")).into(),
                args: vec![
                    Memory(Id::from("arg1")).into(),
                    Memory(Id::from("arg2")).into(),
                ],
                fn_type: FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                ).into()
            }.into(),
            check_null: true
        },
        "if (call2 == nullptr) { call2 = call1->clone();  dynamic_cast<FnT<Int,Int,Int>>(call2)->args = std::make_tuple(arg1, arg2); dynamic_cast<FnT<Int,Int,Int>>(call2)->call(); }";
        "custom fn call"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("e")),
            value: TupleExpression(Vec::new()).into(),
            check_null: false
        },
        "e = std::make_tuple();";
        "empty tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("t")),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: 5}.into())
            ]).into(),
            check_null: false
        },
        "t = std::make_tuple(5LL);";
        "singleton tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("t")),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: -4}.into()),
                Memory(Id::from("y")).into()
            ]).into(),
            check_null: false
        },
        "t = std::make_tuple(-4LL,y);";
        "double tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("bull")),
            value: ConstructorCall {
                idx: 1,
                data: None
            }.into(),
            check_null: false
        },
        "bull = {}; bull.tag = 1ULL;";
        "empty constructor assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("wrapper")),
            value: ConstructorCall {
                idx: 0,
                data: Some((Name::from("Wrapper"), Value::BuiltIn(Integer{value: 4}.into())))
            }.into(),
            check_null: false
        },
        "wrapper = {}; reinterpret_cast<Wrapper*>(&wrapper.value)->value = create_references<Wrapper::type>(4LL); wrapper.tag = 0ULL;";
        "wrapper constructor assignment"
    )]
    fn test_assignment_translation(assignment: Assignment, expected: &str) {
        let code = TRANSLATOR.translate_assignment(assignment);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        MatchStatement{
            expression: (Memory(Id::from("bull")).into(), UnionType(vec![Name::from("Twoo"), Name::from("Faws")]).into()),
            branches: vec![
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into()).into(), AtomicType(AtomicTypeEnum::BOOL).into()),
                            check_null: true
                        }.into(),
                    ],
                },
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Wrap(Value::BuiltIn(Boolean{value: false}.into()).into(), AtomicType(AtomicTypeEnum::BOOL).into()),
                            check_null: true
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
            expression: (
                Memory(Id::from("either")).into(),
                UnionType(vec![Name::from("Left"), Name::from("Right")]).into()
            ),
            branches: vec![
                MatchBranch {
                    target: Some(Memory(Name::from("x"))),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("call")).into(),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Comparison_GE__BuiltIn"),
                                ).into(),
                                fn_type: FnType(
                                    vec![
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                    ],
                                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::BOOL).into()))),
                                ),
                                args: vec![
                                    Memory(Id::from("x")).into(),
                                    Memory(Id::from("y")).into(),
                                ]
                            }.into(),
                            check_null: true
                        }.into()
                    ],
                },
                MatchBranch {
                    target: Some(Memory(Name::from("x"))),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("call")).into(),
                            value: Value::from(Memory(Id::from("x"))).into(),
                            check_null: true
                        }.into(),
                    ],
                }
            ]
        },
        "switch (either.tag) {case 0ULL: { Lazy<destroy_references_t<Left::type>> *x = new LazyConstant<destroy_references_t<Left::type>>{destroy_references(reinterpret_cast<Left*>(&either.value)->value)}; if (call==nullptr){ call=new Comparison_GE__BuiltIn{}; dynamic_cast<FnT<Bool,Int,Int>>(call)->args = std::make_tuple(x,y); dynamic_cast<FnT<Bool,Int,Int>>(call)->call(); } break; } case 1ULL:{ Lazy<destroy_references_t<Right::type>>*x = new LazyConstant<destroy_references_t<Right::type>>{destroy_references(reinterpret_cast<Right*>(&either.value)->value)}; if (call==nullptr){ call=x; } break; }}";
        "match statement read values"
    )]
    #[test_case(
        MatchStatement {
            expression: (Memory(Id::from("nat")).into(), UnionType(vec![Name::from("Suc"), Name::from("Nil")])),
            branches: vec![
                MatchBranch {
                    target: Some(Memory(Name::from("s"))),
                    statements: vec![
                        Declaration {
                            memory: Memory(Id::from("t")),
                            type_: MachineType::Reference(Box::new(UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("t")),
                            value: Expression::Unwrap(Memory(Name::from("s")).into()),
                            check_null: false
                        }.into(),
                        Declaration {
                            memory: Memory(Id::from("u")),
                            type_: UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Wrap(Memory(Name::from("u")).into(), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()),
                            check_null: true
                        }.into(),
                    ],
                },
                MatchBranch {
                    target: None,
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Wrap(Memory(Name::from("nil")).into(), UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into()),
                            check_null: true
                        }.into(),
                    ],
                }
            ]
        },
        "switch (nat.tag) { case 0ULL: { Lazy<destroy_references_t<Suc::type>> *s = new LazyConstant<destroy_references_t<Suc::type>>{destroy_references(reinterpret_cast<Suc*>(&nat.value)->value)}; VariantT<Suc,Nil> *t; VariantT<Suc,Nil> u; t = s->value(); if (r==nullptr){ r=new LazyConstant<VariantT<Suc,Nil>>{u}; } break; } case 1ULL: {if (r==nullptr){ r=new LazyConstant<VariantT<Suc,Nil>>{nil}; } break; }}";
        "match statement recursive type"
    )]
    fn test_match_statement_translation(match_statement: MatchStatement, expected: &str) {
        let code = TRANSLATOR.translate_match_statement(match_statement);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Await(vec![Memory(Id::from("z"))]).into(),
        "WorkManager::await(z);";
        "await for single memory"
    )]
    #[test_case(
        Await(vec![
            Memory(Id::from("z")),
            Memory(Id::from("x"))
        ]).into(),
        "WorkManager::await(z,x);";
        "await for multiple memory"
    )]
    #[test_case(
        IfStatement {
            condition: Memory(Id::from("z")).into(),
            branches: (
                vec![Assignment {
                    target: Memory(Id::from("x")),
                    value: Expression::Wrap(Value::BuiltIn(Integer{value: 1}.into()).into(),  AtomicType(AtomicTypeEnum::INT).into()),
                    check_null: true
                }.into()],
                vec![Assignment {
                    target: Memory(Id::from("x")).into(),
                    value: Expression::Wrap(Value::BuiltIn(Integer{value: -1}.into()).into(), AtomicType(AtomicTypeEnum::INT).into()),
                    check_null: true
                }.into()],
            )
        }.into(),
        "if (z) { if (x == nullptr) { x = new LazyConstant<Int>{1LL}; } } else { if (x == nullptr) { x = new LazyConstant<Int>{-1LL}; } }";
        "if-else statement"
    )]
    #[test_case(
        IfStatement {
            condition: Memory(Id::from("z")).into(),
            branches: (
                vec![
                    IfStatement {
                        condition: Memory(Id::from("y")).into(),
                        branches: (
                            vec![
                                Assignment {
                                    target: Memory(Id::from("x")).into(),
                                    value: Expression::Wrap(Value::BuiltIn(Integer{value: 1}.into()).into(), AtomicType(AtomicTypeEnum::INT).into()),
                                    check_null: true
                                }.into()
                            ],
                            vec![
                                Assignment {
                                    target: Memory(Id::from("x")).into(),
                                    value: Expression::Wrap(Value::BuiltIn(Integer{value: -1}.into()).into(), AtomicType(AtomicTypeEnum::INT).into()),
                                    check_null: true
                                }.into()
                            ],
                        )
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("r")).into(),
                        value: Expression::Wrap(Value::BuiltIn(Boolean{value: true}.into()).into(), AtomicType(AtomicTypeEnum::BOOL).into()),
                        check_null: true
                    }.into(),
                ],
                vec![
                    Assignment {
                        target: Memory(Id::from("x")).into(),
                        value: Expression::Wrap(Value::BuiltIn(Integer{value: 0}.into()).into(), AtomicType(AtomicTypeEnum::INT).into()),
                        check_null: true
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("r")).into(),
                        value: Expression::Wrap(Value::BuiltIn(Boolean{value: false}.into()).into(), AtomicType(AtomicTypeEnum::BOOL).into()),
                        check_null: true
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
        vec![Assignment {
            target: Memory(Id::from("closure")).into(),
            value: ClosureInstantiation{
                name: Name::from("Closure"),
                env: None
            }.into(),
            check_null: true
        }.into()],
        "if (closure==nullptr) { closure=new Closure{}; }";
        "closure without env assignment"
    )]
    #[test_case(
        vec![Assignment {
            target: Memory(Id::from("closure")).into(),
            value: ClosureInstantiation{
                name: Name::from("Adder"),
                env: Some(Memory(Id::from("x")).into())
            }.into(),
            check_null: true
        }.into()],
        "if (closure == nullptr) { closure = new Adder{}; } dynamic_cast<Adder*>(closure)->env = x;";
        "closure assignment"
    )]
    #[test_case(
        vec![
            Await(vec![
                Memory(Id::from("t"))
            ]).into(),
            Declaration {
                type_: TupleType(vec![AtomicType(AtomicTypeEnum::INT).into(), AtomicType(AtomicTypeEnum::INT).into()]).into(),
                memory: Memory(Id::from("tuple"))
            }.into(),
            Assignment {
                target: Memory(Id::from("tuple")),
                value: Expression::Unwrap(
                    Memory(Id::from("t")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                type_: AtomicType(AtomicTypeEnum::INT).into(),
                memory: Memory(Id::from("x"))
            }.into(),
            Assignment {
                target: Memory(Id::from("x")),
                value: ElementAccess{
                    value: Memory(Id::from("tuple")).into(),
                    idx: 1
                }.into(),
                check_null: false
            }.into()
        ],
        "TupleT<Int,Int> tuple; Int x; WorkManager::await(t); tuple = t->value(); x = std::get<1ULL>(tuple);";
        "tuple access"
    )]
    #[test_case(
        vec![
            Declaration{
                type_: MachineType::Reference(Box::new(MachineType::NamedType(Name::from("List")))).into(),
                memory:  Memory(Id::from("tail")),
            }.into(),
            Assignment {
                target: Memory(Id::from("tail")),
                value: ElementAccess{
                    value: Memory(Id::from("cons")).into(),
                    idx: 1
                }.into(),
                check_null: false
            }.into(),
            Declaration{
                type_: UnionType(vec![Name::from("Cons"), Name::from("Nil")]).into(),
                memory: Memory(Id::from("tail"))
            }.into(),
        ],
        "List *tail; VariantT<Cons,Nil> tail; tail = std::get<1ULL>(cons);";
        "cons extraction"
    )]
    #[test_case(
        vec![
            Declaration{
                type_: UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into(),
                memory: Memory(Id::from("n"))
            }.into(),
            Assignment {
                target: Memory(Id::from("n")).into(),
                value: ConstructorCall{
                    idx: 1,
                    data: None
                }.into(),
                check_null: false
            }.into(),
            Declaration{
                type_:UnionType(vec![Name::from("Suc"), Name::from("Nil")]).into(),
                memory: Memory(Id::from("s"))
            }.into(),
            Assignment {
                target: Memory(Id::from("s")).into(),
                value: ConstructorCall{
                    idx: 0,
                    data: Some((
                        Name::from("Suc"),
                        Memory(Id::from("n")).into()
                    ))
                }.into(),
                check_null: false
            }.into(),
        ],
        "VariantT<Suc, Nil> n; VariantT<Suc, Nil> s; n = {}; n.tag = 1ULL; s= {}; reinterpret_cast<Suc *>(&s.value)->value = create_references<Suc::type>(n); s.tag = 0ULL;";
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
            arguments: vec![(Memory(Id::from("x")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))],
            statements: Vec::new(),
            ret: (Memory(Id::from("x")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            allocations: Vec::new()
        },
        "struct IdentityInt : Closure<IdentityInt, Empty, Int, Int> { using Closure<IdentityInt, Empty, Int, Int>::Closure; Lazy<Int> *body(Lazy<Int> *&x) override { return x; } };";
        "identity int"
    )]
    #[test_case(
        FnDef {
            env: None,
            name: Name::from("FourWayPlus"),
            arguments: vec![
                (Memory(Id::from("a")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Memory(Id::from("b")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Memory(Id::from("c")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                (Memory(Id::from("d")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            ],
            statements: vec![
                Assignment {
                    target: Memory(Id::from("call1")),
                    check_null: true,
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                            Memory(Id::from("b")).into(),
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("call2")),
                    check_null: true,
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ),
                        args: vec![
                            Memory(Id::from("c")).into(),
                            Memory(Id::from("d")).into()
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("call3")),
                    check_null: true,
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ),
                        args: vec![
                            Memory(Id::from("call1")).into(),
                            Memory(Id::from("call2")).into(),
                        ]
                    }.into()
                }.into(),
            ],
            ret: (Memory(Id::from("call3")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            allocations: vec![
                Declaration {
                    memory: Memory(Id::from("call1")),
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                },
                Declaration {
                    memory: Memory(Id::from("call2")),
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                },
                Declaration {
                    memory: Memory(Id::from("call3")),
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                }
            ]
        },
        "struct FourWayPlus : Closure<FourWayPlus, Empty, Int, Int, Int, Int, Int> { using Closure<FourWayPlus, Empty, Int, Int, Int, Int, Int>::Closure; FnT<Int,Int,Int> call1 = nullptr; FnT<Int,Int,Int> call2 = nullptr; FnT<Int,Int,Int> call3 = nullptr; Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c, Lazy<Int> *&d) override { if (call1 == nullptr) { call1 = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(call1)->args = std::make_tuple(a, b); dynamic_cast<FnT<Int,Int,Int>>(call1)->call(); } if (call2 == nullptr) { call2 = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(call2)->args = std::make_tuple(c, d); dynamic_cast<FnT<Int,Int,Int>>(call2)->call(); } if (call3 == nullptr) { call3 = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(call3)->args = std::make_tuple(call1, call2); dynamic_cast<FnT<Int,Int,Int>>(call3)->call(); } return call3; } };";
        "four way plus int"
    )]
    #[test_case(
        FnDef {
            env: None,
            name: Name::from("FlatBlockExample"),
            arguments: vec![
                (Memory(Id::from("x")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            ],
            statements: vec![
                Declaration {
                    memory: Memory(Id::from("block_tmp")),
                    type_: FnType(
                        Vec::new(),
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("block_tmp")),
                    value: Block{
                        statements: vec![
                            Assignment {
                                check_null: true,
                                target: Memory(Id::from("call")),
                                value: FnCall{
                                    fn_: BuiltIn::BuiltInFn(
                                        Name::from("Increment__BuiltIn"),
                                    ).into(),
                                    fn_type: FnType(
                                        vec![
                                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                        ],
                                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                    ),
                                    args: vec![
                                        Memory(Id::from("x")).into()
                                    ]
                                }.into()
                            }.into(),
                        ],
                        ret: (Memory(Id::from("call")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))
                    }.into(),
                    check_null: false,
                }.into(),
                Assignment {
                    target: Memory(Id::from("block")),
                    check_null: true,
                    value: FnCall{
                        fn_: Memory(Id::from("block_tmp")).into(),
                        fn_type: FnType(
                            Vec::new(),
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ),
                        args: Vec::new()
                    }.into()
                }.into(),
            ],
            ret: (Memory(Id::from("block")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            allocations: vec![
                Declaration{
                    memory: Memory(Id::from("block")),
                    type_: FnType(
                        Vec::new(),
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                },
                Declaration{
                    memory: Memory(Id::from("call")),
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                }
            ]
        },
        "struct FlatBlockExample : Closure<FlatBlockExample, Empty, Int, Int> { using Closure<FlatBlockExample, Empty, Int, Int>::Closure; FnT<Int> block = nullptr; FnT<Int, Int> call = nullptr; Lazy<Int> *body(Lazy<Int> *&x) override { FnT<Int> block_tmp; block_tmp = new BlockFn<Int>([&]() { if (call == nullptr) { call = new Increment__BuiltIn {}; dynamic_cast<FnT<Int,Int>>(call)->args = std::make_tuple(x); dynamic_cast<FnT<Int,Int>>(call)->call(); } return call; }); if (block == nullptr) { block = block_tmp->clone(); dynamic_cast<FnT<Int>>(block)->args = std::make_tuple(); dynamic_cast<FnT<Int>>(block)->call(); } return block; } };";
        "flat block example"
    )]
    #[test_case(
        FnDef{
            env: Some(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            name: Name::from("Adder"),
            arguments: vec![(Memory(Id::from("x")), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())))],
            statements: vec![
                Assignment {
                    target: Memory(Id::from("inner_res")),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            ],
                            Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                            Memory(Id::from("env")).into(),
                        ]
                    }.into(),
                    check_null: true
                }.into()
            ],
            ret: (Memory(Id::from("inner_res")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
            allocations: vec![
                Declaration{
                    memory: Memory(Id::from("inner_res")),
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                            MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    ).into()
                }
            ]
        },
    "struct Adder : Closure<Adder, Lazy<Int> *, Int, Int> { using Closure<Adder, Lazy<Int> *, Int, Int>::Closure; FnT<Int, Int, Int> inner_res = nullptr; Lazy<Int> *body(Lazy<Int> *&x) override { if (inner_res == nullptr) { inner_res = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(inner_res)->args = std::make_tuple(x, env); dynamic_cast<FnT<Int,Int,Int>>(inner_res)->call(); } return inner_res; } };";
    "adder closure"
    )]
    fn test_fn_def_translation(fn_def: FnDef, expected: &str) {
        let code = TRANSLATOR.translate_fn_def(fn_def);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Program {
            type_defs: vec![
                TypeDef{
                    name: Name::from("Bull"),
                    constructors: vec![
                        (Name::from("Twoo"), None),
                        (Name::from("Faws"), None)
                    ]
                }
            ],
            fn_defs: vec![
                FnDef {
                    env: None,
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("call")),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Plus__BuiltIn"),
                                ).into(),
                                fn_type: FnType(
                                    vec![
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                        MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                                    ],
                                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                ),
                                args: vec![
                                    Memory(Id::from("x")).into(),
                                    Memory(Id::from("y")).into(),
                                ]
                            }.into(),
                            check_null: true
                        }.into(),
                    ],
                    ret: (Memory(Id::from("call")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    allocations: vec![
                        Declaration{
                            memory: Memory(Id::from("call")),
                            type_: FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into())),
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        }
                    ]
                },
                FnDef {
                    env: None,
                    name: Name::from("PreMain"),
                    arguments: Vec::new(),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("x")).into(),
                            value: Expression::Wrap(Value::BuiltIn(Integer{value: 9}.into()),AtomicType(AtomicTypeEnum::INT).into()),
                            check_null: true
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("y")).into(),
                            value: Expression::Wrap(Value::BuiltIn(Integer{value: 5}.into()),AtomicType(AtomicTypeEnum::INT).into()),
                            check_null: true
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("main")),
                            check_null: true,
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Main"),
                                ).into(),
                                fn_type: FnType(
                                    Vec::new(),
                                    Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                                ),
                                args: Vec::new()
                            }.into()
                        }.into(),
                    ],
                    ret: (Memory(Id::from("main")).into(), MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                    allocations: vec![
                        Declaration{
                            memory: Memory(Id::from("main")),
                            type_: FnType(
                                Vec::new(),
                                Box::new(MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))),
                            ).into()
                        }
                    ]
                }
            ],
        },
        "#include \"main/include.hpp\"\nstruct Twoo; struct Faws; typedef VariantT<Twoo, Faws> Bull; struct Twoo{Empty value;}; struct Faws{Empty value;}; struct Main : Closure<Main, Empty, Int> { using Closure<Main, Empty, Int>::Closure; FnT<Int, Int, Int> call = nullptr; Lazy<Int> *body() override { if (call == nullptr){ call = new Plus__BuiltIn{}; dynamic_cast<FnT<Int,Int,Int>>(call)->args = std::make_tuple(x,y); dynamic_cast<FnT<Int,Int,Int>>(call)->call(); } return call; } }; struct PreMain : Closure<PreMain, Empty, Int> { using Closure<PreMain, Empty, Int>::Closure; FnT<Int> main = nullptr; Lazy<Int> *body() override { if (x == nullptr) {x = new LazyConstant<Int>{9LL};} if (y == nullptr) {y = new LazyConstant<Int>{5LL}; }if (main == nullptr){ main = new Main{}; dynamic_cast<FnT<Int>>(main)->args = std::make_tuple(); dynamic_cast<FnT<Int>>(main)->call(); } return main; } }; ";
        "main program"
    )]
    fn test_program_translation(program: Program, expected: &str) {
        let code = Translator::translate(program);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }
}
