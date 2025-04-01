use itertools::Either::{Left, Right};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::convert::identity;

use compilation::{
    Allocation, Assignment, Await, Boolean, BuiltIn, ClosureInstantiation, ConstructorCall,
    Declaration, ElementAccess, Expression, FnCall, FnDef, Id, IfStatement, Integer, MachineType,
    MatchStatement, Memory, Name, Program, Statement, TupleExpression, TupleType, TypeDef,
    UnionType, Value,
};

use crate::type_formatter::TypeFormatter;

type Code = String;

pub struct Emitter {}

impl Emitter {
    fn emit_type(&self, type_: &MachineType) -> Code {
        format!("{}", TypeFormatter(type_))
    }
    fn emit_lazy_type(&self, type_: &MachineType) -> Code {
        format!("LazyT<{}>", self.emit_type(type_))
    }
    // Topologically sort types to ensure consistency in definitions.
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
            // Find any used types based on constructors.
            let type_name = type_name
                .split_once("C")
                .map_or(type_name.clone(), |(before, _)| String::from(before));
            self.top_sort_internal(type_defs[&type_name].clone(), type_defs, visited, result);
        }
        result.extend(type_def.constructors.iter().map(|ctor| ctor.clone()));
    }
    fn emit_type_defs(&self, type_defs: Vec<TypeDef>) -> Code {
        // Predefine any used types (without information).
        let forward_constructor_definitions = type_defs
            .iter()
            .map(|type_def| {
                type_def
                    .constructors
                    .iter()
                    .map(|constructor| format!("struct {};", constructor.0))
            })
            .flatten();
        // Define type aliases for structs.
        let type_definitions = type_defs.iter().map(|type_def| {
            let variant_definition = self.emit_type(&MachineType::UnionType(UnionType(
                type_def
                    .constructors
                    .iter()
                    .map(|constructor| constructor.0.clone())
                    .collect_vec(),
            )));
            format!("typedef {variant_definition} {};", type_def.name)
        });
        // Define constructors.
        let ctors = self.top_sort(&type_defs);
        let constructor_definitions = ctors.iter().map(|(name, type_)| {
            let fields = match type_ {
                Some(type_) => {
                    format!(
                        "using type = {}; {} value;",
                        self.emit_type(type_),
                        self.emit_lazy_type(&MachineType::NamedType(Name::from("type")))
                    )
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
    fn emit_value_type(&self, value: &Value) -> Code {
        match value {
            Value::BuiltIn(BuiltIn::Boolean(_)) => Code::from("Bool"),
            Value::BuiltIn(BuiltIn::Integer(_)) => Code::from("Int"),
            Value::BuiltIn(BuiltIn::BuiltInFn(name)) => format!("decltype({name}_G)"),
            Value::Memory(Memory(id)) => format!("decltype({id})"),
        }
    }
    fn emit_builtin(&self, value: BuiltIn) -> Code {
        let value_type = self.emit_value_type(&value.clone().into());
        match value {
            BuiltIn::Integer(Integer { value }) => {
                format!("{value_type}{{{value}LL}}")
            }
            BuiltIn::Boolean(Boolean { value }) => {
                format!("{value_type}{{{value}}}")
            }
            BuiltIn::BuiltInFn(name) => {
                format!("make_lazy<{value_type}>({name}_G)")
            }
        }
    }
    fn emit_memory(&self, Memory(id): Memory) -> Code {
        id
    }
    fn emit_value(&self, value: Value) -> Code {
        match value {
            Value::BuiltIn(value) => self.emit_builtin(value),
            Value::Memory(memory) => self.emit_memory(memory),
        }
    }
    fn emit_value_list(&self, values: Vec<Value>) -> Code {
        values
            .into_iter()
            .map(|value| self.emit_value(value))
            .join(", ")
    }
    fn emit_expression(&self, expression: Expression) -> Code {
        match expression {
            Expression::ElementAccess(ElementAccess { value, idx }) => {
                let code = format!("std::get<{idx}ULL>({})", self.emit_value(value.clone()));
                if Value::Memory(Memory(Id::from("env"))) == value {
                    format!("load_env({code})")
                } else {
                    code
                }
            }
            Expression::Value(value) => self.emit_value(value),
            Expression::TupleExpression(TupleExpression(values)) => {
                format!("std::make_tuple({})", self.emit_value_list(values))
            }
            Expression::ConstructorCall(constructor_call) => {
                self.emit_constructor_call(constructor_call)
            }
            Expression::FnCall(fn_call) => self.emit_fn_call(fn_call),
            e => panic!("{:?} does not emit directly as an expression", e),
        }
    }
    fn emit_await(&self, await_: Await) -> Code {
        let arguments = await_
            .0
            .into_iter()
            .map(|memory| self.emit_memory(memory))
            .join(",");
        format!("WorkManager::await({arguments});")
    }
    fn emit_fn_call(&self, fn_call: FnCall) -> Code {
        let args_code = self.emit_value_list(fn_call.args);
        match fn_call.fn_ {
            Value::BuiltIn(built_in) => {
                let BuiltIn::BuiltInFn(name) = built_in else {
                    panic!("Attempt to call non-fn built-in.")
                };
                format!("{name}({args_code})",)
            }
            Value::Memory(memory) => {
                let id = self.emit_memory(memory);
                let call_code = if args_code.len() == 0 {
                    format!("extract_lazy({})", id)
                } else {
                    format!("extract_lazy({}),{}", id, args_code)
                };
                format!("fn_call({call_code})")
            }
        }
    }
    fn emit_constructor_call(&self, constructor_call: ConstructorCall) -> Code {
        let indexing_code = format!(
            "std::integral_constant<std::size_t,{}>()",
            constructor_call.idx
        );
        let value_code = match constructor_call.data {
            None => Code::new(),
            Some((name, value)) => {
                format!(", {name}{{ensure_lazy({})}}", self.emit_value(value))
            }
        };
        let type_ = constructor_call.type_;
        format!("{type_}{{{indexing_code}{value_code}}}")
    }
    fn emit_declaration(&self, declaration: Declaration, declared: &mut HashSet<Memory>) -> Code {
        let Declaration { type_, memory } = declaration;
        declared.insert(memory.clone());
        format!(
            "{} {};",
            self.emit_lazy_type(&type_),
            self.emit_memory(memory)
        )
    }
    /// Define allocator for cyclic closures in fn defs.
    fn emit_allocation(&self, allocation: Allocation) -> (Code, HashMap<Memory, (Code, usize)>) {
        let Allocation { name, fns, target } = allocation;
        let mut allocated_memory = HashMap::new();
        let mut struct_fields = fns.into_iter().enumerate().map(|(i, (memory, name))| {
            let target = self.emit_memory(target.clone());
            allocated_memory.insert(memory.clone(), (target.clone(), i));
            format!("ClosureFnT<remove_lazy_t<typename {name}::EnvT>, typename {name}::Fn> _{i};")
        });
        let struct_definition = format!("struct {name} {{ {} }};", struct_fields.join("\n"));
        let instantiation = format!(
            "std::shared_ptr<{name}> {} = std::make_shared<{name}>();",
            self.emit_memory(target)
        );
        (
            format!("{struct_definition} {instantiation}"),
            allocated_memory,
        )
    }
    fn emit_assignment(&self, assignment: Assignment, declared: &HashSet<Memory>) -> Code {
        let Memory(id) = assignment.target.clone();
        let value_code = match assignment.value {
            Expression::ClosureInstantiation(ClosureInstantiation { name, env }) => {
                return env.map_or_else(|| format!("{id} = make_lazy<remove_lazy_t<decltype({id})>>({name}::G);"), |env| {
                    let value = self.emit_value(env);
                    format!(
                        "std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename {name}::EnvT>,remove_shared_ptr_t<remove_lazy_t<decltype({id})>>>>({id}->lvalue())->env = store_env<typename {name}::EnvT>({value});",
                    )
                })
            }
            value => self.emit_expression(value),
        };
        if declared.contains(&assignment.target) {
            format!("{id} = ensure_lazy({value_code});")
        } else {
            // Use C++'s type inference system to determine whether the value is lazy or not.
            format!("auto {id} = {value_code};")
        }
    }
    fn emit_if_statement(&self, if_statement: IfStatement, declared: HashSet<Memory>) -> Code {
        let condition_code = self.emit_value(if_statement.condition);
        let if_branch = self.emit_statements(if_statement.branches.0, declared.clone());
        let else_branch = self.emit_statements(if_statement.branches.1, declared.clone());
        format!("if (extract_lazy({condition_code})) {{ {if_branch} }} else {{ {else_branch} }}",)
    }
    fn emit_match_statement(
        &self,
        match_statement: MatchStatement,
        declared: HashSet<Memory>,
    ) -> Code {
        let UnionType(types) = match_statement.expression.1;
        let subject = self.emit_memory(match_statement.auxiliary_memory);
        let extraction = format!(
            "auto {subject} = extract_lazy({});",
            self.emit_value(match_statement.expression.0)
        );
        let branches_code = match_statement
            .branches
            .into_iter()
            .enumerate()
            .map(|(i, branch)| {
                let assignment_code = match branch.target {
                    Some(Memory(id)) => {
                        let type_name = &types[i];
                        format!(
                            "{} {id} = reinterpret_cast<{type_name}*>(&{subject}.value)->value;",
                            self.emit_lazy_type(&MachineType::NamedType(format!(
                                "{type_name}::type"
                            )))
                        )
                    }
                    None => Code::new(),
                };
                let statements_code =
                    self.emit_statements(branch.statements.clone(), declared.clone());
                format!("case {i}ULL : {{ {assignment_code} {statements_code} break; }}",)
            })
            .join("\n");
        format!("{extraction} switch ({subject}.tag) {{ {branches_code} }}")
    }
    fn emit_statement(&self, statement: Statement, declared: &mut HashSet<Memory>) -> Code {
        match statement {
            Statement::Await(await_) => self.emit_await(await_),
            Statement::Assignment(assignment) => self.emit_assignment(assignment, &declared),
            Statement::IfStatement(if_statement) => {
                self.emit_if_statement(if_statement, declared.clone())
            }
            Statement::Declaration(declaration) => self.emit_declaration(declaration, declared),
            Statement::Allocation(allocation) => self.emit_allocation(allocation).0,
            Statement::MatchStatement(match_statement) => {
                self.emit_match_statement(match_statement, declared.clone())
            }
        }
    }
    fn emit_statements(&self, statements: Vec<Statement>, mut declared: HashSet<Memory>) -> Code {
        let (forwarded, other_statements): (Vec<_>, Vec<_>) =
            statements
                .into_iter()
                .partition_map(|statement| match statement {
                    Statement::Declaration(declaration) => Left(Left(declaration)),
                    Statement::Allocation(allocation) => Left(Right(allocation)),
                    statement => Right(statement),
                });

        let (declarations, allocations): (Vec<_>, Vec<_>) =
            forwarded.into_iter().partition_map(identity);

        // Add declarations before any other statements.
        let declarations_code = declarations
            .into_iter()
            .map(|declaration| self.emit_declaration(declaration, &mut declared))
            .join("\n");

        // Add allocations after declarations but before any other statements.
        let (allocation_codes, allocations): (Vec<_>, Vec<_>) = allocations
            .into_iter()
            .map(|allocation| self.emit_allocation(allocation))
            .unzip();
        let allocations_code = allocation_codes.join("\n");
        let allocations: HashMap<Memory, (Code, usize)> =
            HashMap::from_iter(allocations.into_iter().flatten());

        // Setup closures before instantiating them.
        let closure_predefinitions =
            other_statements
                .iter()
                .filter_map(|statement| {
                    if let Statement::Assignment(Assignment {
                        target,
                        value:
                            Expression::ClosureInstantiation(ClosureInstantiation {
                                name,
                                env: Some(_),
                            }),
                    }) = statement
                    {
                        let id = self.emit_memory(target.clone());
                        Some(match allocations.get(target) {
                            Some((allocator, idx)) => format!(
                                "{id} = setup_closure<{name}>({allocator}, {allocator}->_{idx});"
                            ),
                            None => format!("{id} = setup_closure<{name}>();"),
                        })
                    } else {
                        None
                    }
                })
                .join("\n");

        let other_code = other_statements
            .into_iter()
            .map(|statement| self.emit_statement(statement, &mut declared))
            .join("\n");

        format!("{declarations_code}\n{allocations_code}\n{closure_predefinitions}\n{other_code}")
    }
    fn emit_fn_def(&self, fn_def: FnDef) -> Code {
        let name = fn_def.name;
        let return_type = fn_def.ret.1;
        let declared = HashSet::new();
        let statements_code = self.emit_statements(fn_def.statements, declared);
        let return_code = format!("return ensure_lazy({});", self.emit_value(fn_def.ret.0));
        let external_types = &std::iter::once(return_type.clone())
            .chain(fn_def.arguments.iter().map(|(_, type_)| type_.clone()))
            .map(|type_| self.emit_type(&type_))
            .join(",");
        let base_name = "TypedClosureI";
        let (env_ptr, replication_args, base, instance) = if fn_def.env.len() == 0 {
            (
                String::new(),
                String::from("args"),
                format!("{base_name}<Empty,{external_types}>"),
                format!("static inline FnT<{external_types}> G = std::make_shared<TypedClosureG<Empty,{external_types}>>(init);")
            )
        } else {
            (
                String::from(", const EnvT &env"),
                String::from("args, env"),
                format!(
                    "{base_name}<{},{external_types}>",
                    self.emit_type(&TupleType(fn_def.env).into()),
                ),
                String::new(),
            )
        };

        let declaration_code = format!("struct {name} : {base}");
        let constructor_code = format!("using {base}::{base_name};");

        let header_code = format!(
            "{} body({}) override",
            self.emit_lazy_type(&return_type),
            fn_def
                .arguments
                .into_iter()
                .map(|(memory, type_)| format!(
                    "{} &{}",
                    self.emit_lazy_type(&type_),
                    self.emit_memory(memory)
                ))
                .join(",")
        );
        let size_code = format!(
            "constexpr std::size_t lower_size_bound() const override {{ return {}; }}; constexpr std::size_t upper_size_bound() const override {{ return {}; }};",
            fn_def.size_bounds.0,
            fn_def.size_bounds.1,
        );
        let recursive_code = format!(
            "constexpr bool is_recursive() const override {{ return {}; }};",
            fn_def.is_recursive
        );
        let initialization_code = format!(
            "static std::unique_ptr<TypedFnI<{external_types}>> init(const ArgsT &args{env_ptr}) {{ return std::make_unique<{name}>({replication_args}); }}"
        );
        format!(
            "{declaration_code} {{ {constructor_code} {header_code} {{ {statements_code} {return_code} }} {size_code} {recursive_code} {initialization_code} {instance} }};"
        )
    }
    fn emit_fn_defs(&self, fn_defs: Vec<FnDef>) -> Code {
        fn_defs
            .into_iter()
            .map(|fn_def| self.emit_fn_def(fn_def))
            .join("\n")
    }
    fn emit_program(&self, program: Program) -> Code {
        let type_def_code = self.emit_type_defs(program.type_defs);
        let fn_def_code = self.emit_fn_defs(program.fn_defs);
        // Add header with all libraries.
        format!("#include \"main/include.hpp\"\n\n{type_def_code} {fn_def_code}")
    }
    pub fn emit(program: Program) -> Code {
        let emitter = Emitter {};
        emitter.emit_program(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use compilation::{Allocation, AtomicType, AtomicTypeEnum, FnType, Id, MatchBranch, Name};
    use once_cell::sync::Lazy;
    use regex::Regex;
    use test_case::test_case;

    const EMITTER: Lazy<Emitter> = Lazy::new(|| Emitter {});

    /// Remove spaces between non-words for easier equality checking.
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
                AtomicType(AtomicTypeEnum::INT).into(),
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        ).into(),
        "FnT<Bool,Int,Int>";
        "int comparison fn"
    )]
    #[test_case(
        MachineType::WeakFnType(FnType(
            vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::INT).into(),
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        )),
        "WeakFnT<Bool,Int,Int>";
        "weak function"
    )]
    #[test_case(
        FnType(
            vec![
                FnType(
                    vec![
                        AtomicType(AtomicTypeEnum::INT).into(),
                    ],
                    Box::new(AtomicType(AtomicTypeEnum::BOOL).into(),)
                ).into(),
                AtomicType(AtomicTypeEnum::INT).into(),
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into(),)
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
    fn test_type_emission(type_: MachineType, expected: &str) {
        let code = EMITTER.emit_type(&type_);
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
        "struct Left_IntBool; struct Right_IntBool; typedef VariantT<Left_IntBool, Right_IntBool> EitherIntBool; struct Left_IntBool { using type = Int; LazyT<type> value; }; struct Right_IntBool { using type = Bool; LazyT<type> value; };";
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
        "struct Cons_Int; struct Nil_Int; typedef VariantT<Cons_Int, Nil_Int> ListInt; struct Cons_Int{ using type = TupleT<Int,ListInt>; LazyT<type> value;}; struct Nil_Int{ Empty value; };";
        "list int"
    )]
    fn test_typedef_emissions(type_def: TypeDef, expected: &str) {
        let code = EMITTER.emit_type_defs(vec![type_def]);
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
        "struct Basic; struct Complex; struct None; struct Some; typedef VariantT<Basic,Complex> Expression; typedef VariantT<None,Some> Value; struct None{Empty value;}; struct Some { using type = Expression; LazyT<type> value; }; struct Basic { using type = Int; LazyT<type> value; }; struct Complex { using type = TupleT<Value, Value>; LazyT<type> value; };";
        "mutually recursive types"
    )]
    fn test_typedefs_emissions(type_defs: Vec<TypeDef>, expected: &str) {
        let code = EMITTER.emit_type_defs(type_defs);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Integer{value: 24}.into(),
        "Int{24LL}";
        "integer emission"
    )]
    #[test_case(
        Integer{value: -24}.into(),
        "Int{-24LL}";
        "negative integer emission"
    )]
    #[test_case(
        Integer{value: 0}.into(),
        "Int{0LL}";
        "zero emission"
    )]
    #[test_case(
        Integer{value: 10000000000009}.into(),
        "Int{10000000000009LL}";
        "large integer emission"
    )]
    #[test_case(
        Boolean{value: true}.into(),
        "Bool{true}";
        "true emission"
    )]
    #[test_case(
        Boolean{value: false}.into(),
        "Bool{false}";
        "false emission"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Plus__BuiltIn"),
        ),
        "make_lazy<decltype(Plus__BuiltIn_G)>(Plus__BuiltIn_G)";
        "builtin plus emission"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_GE__BuiltIn"),
        ),
        "make_lazy<decltype(Comparison_GE__BuiltIn_G)>(Comparison_GE__BuiltIn_G)";
        "builtin greater than or equal to emission"
    )]
    fn test_builtin_emission(value: BuiltIn, expected: &str) {
        let code = EMITTER.emit_builtin(value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(Memory(Id::from("x")), "x")]
    #[test_case(Memory(Id::from("bar")), "bar")]
    #[test_case(Memory(Id::from("baz")), "baz")]
    fn test_memory_emission(memory: Memory, expected: &str) {
        let code = EMITTER.emit_memory(memory);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Memory(Id::from("baz")).into(),
        "decltype(baz)";
        "value memory type"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_LT__BuiltIn"),
        ).into(),
        "decltype(Comparison_LT__BuiltIn_G)";
        "builtin function type"
    )]
    #[test_case(
        BuiltIn::Integer(Integer{value: -1}).into(),
        "Int";
        "builtin integer type"
    )]
    #[test_case(
        BuiltIn::Boolean(Boolean{value: false}).into(),
        "Bool";
        "builtin boolean type"
    )]
    fn test_value_type(value: Value, expected: &str) {
        let code = EMITTER.emit_value_type(&value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Memory(Id::from("baz")).into(),
        "baz";
        "value memory emission"
    )]
    #[test_case(
        BuiltIn::BuiltInFn(
            Name::from("Comparison_LT__BuiltIn"),
        ).into(),
        "make_lazy<decltype(Comparison_LT__BuiltIn_G)>(Comparison_LT__BuiltIn_G)";
        "builtin function emission"
    )]
    #[test_case(
        BuiltIn::Integer(Integer{value: -1}).into(),
        "Int{-1LL}";
        "builtin integer emission"
    )]
    fn test_value_emission(value: Value, expected: &str) {
        let code = EMITTER.emit_value(value);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Value::BuiltIn(BuiltIn::Integer(Integer{value: -1}).into()).into(),
        "Int{-1LL}";
        "integer"
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
    fn test_expression_emission(expression: Expression, expected: &str) {
        let code = EMITTER.emit_expression(expression);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        Assignment {
            target: Memory(Id::from("x")).into(),
            value: Value::BuiltIn(Integer{value: 5}.into()).into(),
        },
        "auto x = Int{5LL};";
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
        },
        "auto x = std::get<0ULL>(tuple);";
        "tuple access assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("y")).into(),
            value: Value::BuiltIn(Boolean{value: true}.into()).into(),
        },
        "auto y = Bool{true};";
        "boolean assignment"
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
                        AtomicType(AtomicTypeEnum::INT).into(),
                        AtomicType(AtomicTypeEnum::INT).into()
                    ],
                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                ),
                args: vec![
                    Memory(Id::from("arg1")).into(),
                    Memory(Id::from("arg2")).into(),
                ]
            }.into(),
        },
        "auto call = Plus__BuiltIn(arg1, arg2);";
        "built-in fn call"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("res")),
            value: FnCall{
                fn_: Memory(Name::from("call")).into(),
                args: Vec::new(),
                fn_type: FnType(
                    Vec::new(),
                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                ).into()
            }.into(),
        },
        "auto res = fn_call(extract_lazy(call));";
        "custom fn call no args"
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
                        AtomicType(AtomicTypeEnum::INT).into(),
                        AtomicType(AtomicTypeEnum::INT).into(),
                    ],
                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                ).into()
            }.into(),
        },
        "auto call2 = fn_call(extract_lazy(call1), arg1, arg2);";
        "custom fn call"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("e")),
            value: TupleExpression(Vec::new()).into(),
        },
        "auto e = std::make_tuple();";
        "empty tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("t")),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: 5}.into())
            ]).into(),
        },
        "auto t = std::make_tuple(Int{5LL});";
        "singleton tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("t")),
            value: TupleExpression(vec![
                Value::BuiltIn(Integer{value: -4}.into()),
                Memory(Id::from("y")).into()
            ]).into(),
        },
        "auto t = std::make_tuple(Int{-4LL},y);";
        "double tuple assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("bull")),
            value: ConstructorCall {
                type_: Name::from("Bull"),
                idx: 1,
                data: None
            }.into(),
        },
        "auto bull = Bull{std::integral_constant<std::size_t,1>()};";
        "empty constructor assignment"
    )]
    #[test_case(
        Assignment {
            target: Memory(Id::from("wrapper")),
            value: ConstructorCall {
                type_: Name::from("Wrapper"),
                idx: 0,
                data: Some((Name::from("Wrapper"), Value::BuiltIn(Integer{value: 4}.into())))
            }.into(),
        },
        "auto wrapper = Wrapper{std::integral_constant<std::size_t,0>(), Wrapper{ensure_lazy(Int{4LL})}};";
        "wrapper constructor assignment"
    )]
    fn test_assignment_emission(assignment: Assignment, expected: &str) {
        let code = EMITTER.emit_assignment(assignment, &HashSet::new());
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        vec![Await(vec![Memory(Id::from("z"))]).into()],
        "WorkManager::await(z);";
        "await for single memory"
    )]
    #[test_case(
        vec![
            Await(vec![
                Memory(Id::from("z")),
                Memory(Id::from("x"))
            ]).into()
        ],
        "WorkManager::await(z,x);";
        "await for multiple memory"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("z"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::INT.into(),
                memory: Memory(Id::from("x"))
            }.into(),
            IfStatement {
                condition: Memory(Id::from("z")).into(),
                branches: (
                    vec![Assignment {
                        target: Memory(Id::from("x")),
                        value: Expression::Value(Value::BuiltIn(Integer{value: 1}.into()).into()),
                    }.into()],
                    vec![Assignment {
                        target: Memory(Id::from("x")).into(),
                        value: Expression::Value(Value::BuiltIn(Integer{value: -1}.into()).into()),
                    }.into()],
                )
            }.into()
        ],
        "LazyT<Int> x; WorkManager::await(z); if (extract_lazy(z)) { x = ensure_lazy(Int{1LL}); } else { x = ensure_lazy(Int{-1LL}); }";
        "if-else statement"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("z"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::INT.into(),
                memory: Memory(Id::from("x"))
            }.into(),
            Declaration {
                type_: AtomicTypeEnum::BOOL.into(),
                memory: Memory(Id::from("r"))
            }.into(),
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
                                        value: Expression::Value(Value::BuiltIn(Integer{value: 1}.into()).into()),
                                    }.into()
                                ],
                                vec![
                                    Assignment {
                                        target: Memory(Id::from("x")).into(),
                                        value: Expression::Value(Value::BuiltIn(Integer{value: -1}.into()).into()),
                                    }.into()
                                ],
                            )
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Value(Value::BuiltIn(Boolean{value: true}.into()).into()),
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("x")).into(),
                            value: Expression::Value(Value::BuiltIn(Integer{value: 0}.into()).into()),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("r")).into(),
                            value: Expression::Value(Value::BuiltIn(Boolean{value: false}.into()).into()),
                        }.into(),
                    ],
                )
            }.into()
        ],
        "LazyT<Int> x; LazyT<Bool> r; WorkManager::await(z); if (extract_lazy(z)) { if (extract_lazy(y)) { x = ensure_lazy(Int{1LL}); } else { x = ensure_lazy(Int{-1LL}); } r = ensure_lazy(Bool{true}); } else { x = ensure_lazy(Int{0LL}); r = ensure_lazy(Bool{false}); }";
        "nested if-else statement"
    )]
    #[test_case(
        vec![Assignment {
            target: Memory(Id::from("closure")).into(),
            value: ClosureInstantiation{
                name: Name::from("Closure"),
                env: None
            }.into(),
        }.into()],
        "closure = make_lazy<remove_lazy_t<decltype(closure)>>(Closure::G);";
        "closure without env assignment"
    )]
    #[test_case(
        vec![Assignment {
            target: Memory(Id::from("closure")).into(),
            value: ClosureInstantiation{
                name: Name::from("Adder"),
                env: Some(Memory(Id::from("x")).into())
            }.into(),
        }.into()],
        "closure = setup_closure<Adder>(); std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename Adder::EnvT>,remove_shared_ptr_t<remove_lazy_t<decltype(closure)>>>>(closure->lvalue())->env = store_env<typename Adder::EnvT>(x);";
        "closure assignment"
    )]
    #[test_case(
        vec![
            Declaration {
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ).into(),
                memory: Memory(Id::from("is_odd_fn"))
            }.into(),
            Declaration {
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ).into(),
                memory: Memory(Id::from("is_even_fn"))
            }.into(),
            Allocation{
                name: Name::from("Allocator"),
                target: Memory(Id::from("allocator")),
                fns: vec![
                    (Memory(Id::from("is_odd_fn")), Name::from("IsOdd")),
                    (Memory(Id::from("is_even_fn")), Name::from("IsEven")),
                ]
            }.into(),
            Assignment {
                target: Memory(Id::from("is_odd_fn")).into(),
                value: ClosureInstantiation{
                    name: Name::from("IsOdd"),
                    env: Some(Memory(Id::from("is_odd_env")).into())
                }.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("is_even_fn")).into(),
                value: ClosureInstantiation{
                    name: Name::from("IsEven"),
                    env: Some(Memory(Id::from("is_even_env")).into())
                }.into(),
            }.into(),
        ],
        "LazyT<FnT<Bool, Int>> is_odd_fn; LazyT<FnT<Bool, Int>> is_even_fn; struct Allocator { ClosureFnT<remove_lazy_t<typename IsOdd::EnvT>,typename IsOdd::Fn> _0; ClosureFnT<remove_lazy_t<typename IsEven::EnvT>,typename IsEven::Fn> _1; }; std::shared_ptr<Allocator> allocator = std::make_shared<Allocator>(); is_odd_fn = setup_closure<IsOdd>(allocator,allocator->_0); is_even_fn = setup_closure<IsEven>(allocator,allocator->_1); std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename IsOdd::EnvT>,remove_shared_ptr_t<remove_lazy_t<decltype(is_odd_fn)>>>>(is_odd_fn->lvalue())->env = store_env<typename IsOdd::EnvT>(is_odd_env); std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename IsEven::EnvT>, remove_shared_ptr_t<remove_lazy_t<decltype(is_even_fn)>>>>( is_even_fn->lvalue())->env = store_env<typename IsEven::EnvT>(is_even_env);";
        "mutually recursive closure assignment"
    )]
    #[test_case(
        vec![
            Declaration {
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into(),
                memory: Memory(Id::from("f1"))
            }.into(),
            Allocation{
                name: Name::from("alloc23"),
                fns: vec![
                    (Memory(Id::from("f2")), Name::from("F2")),
                    (Memory(Id::from("f3")), Name::from("F3")),
                ],
                target: Memory(Id::from("a23"))
            }.into(),
            Allocation{
                name: Name::from("alloc45"),
                fns: vec![
                    (Memory(Id::from("f4")), Name::from("F4")),
                    (Memory(Id::from("f4")), Name::from("F5")),
                ],
                target: Memory(Id::from("a45"))
            }.into(),
            Assignment {
                target: Memory(Id::from("f4")).into(),
                value: ClosureInstantiation{
                    name: Name::from("F4"),
                    env: Some(Memory(Id::from("f4_env")).into())
                }.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("f1")).into(),
                value: ClosureInstantiation{
                    name: Name::from("F1"),
                    env: Some(Memory(Id::from("f1_env")).into())
                }.into(),
            }.into()
        ],
        "LazyT<FnT<Int,Int>>f1; struct alloc23 { ClosureFnT<remove_lazy_t<typename F2::EnvT>,typename F2::Fn>_0; ClosureFnT<remove_lazy_t<typename F3::EnvT>,typename F3::Fn>_1; }; std::shared_ptr<alloc23>a23 = std::make_shared<alloc23>(); struct alloc45{ClosureFnT<remove_lazy_t<typename F4::EnvT>,typename F4::Fn> _0; ClosureFnT<remove_lazy_t<typename F5::EnvT>,typename F5::Fn> _1; }; std::shared_ptr<alloc45>a45 = std::make_shared<alloc45>(); f4 = setup_closure<F4>(a45,a45->_1); f1 = setup_closure<F1>(); std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename F4::EnvT>,remove_shared_ptr_t<remove_lazy_t<decltype(f4)>>>>(f4->lvalue())->env = store_env<typename F4::EnvT>(f4_env); std::dynamic_pointer_cast<ClosureFnT<remove_lazy_t<typename F1::EnvT>,remove_shared_ptr_t<remove_lazy_t<decltype(f1)>>>>(f1->lvalue())->env = store_env<typename F1::EnvT>(f1_env); ";
        "dual allocator"
    )]
    #[test_case(
        vec![
            Await(vec![
                Memory(Id::from("t"))
            ]).into(),
            Assignment {
                target: Memory(Id::from("x")),
                value: ElementAccess{
                    value: Memory(Id::from("tuple")).into(),
                    idx: 1
                }.into(),
            }.into()
        ],
        "WorkManager::await(t); auto x = std::get<1ULL>(tuple);";
        "tuple access"
    )]
    #[test_case(
        vec![
            Assignment {
                target: Memory(Id::from("f")),
                value: ElementAccess{
                    value: Memory(Id::from("env")).into(),
                    idx: 1
                }.into(),
            }.into()
        ],
        "auto f = load_env(std::get<1ULL>(env));";
        "env access"
    )]
    #[test_case(
        vec![
            Assignment {
                target: Memory(Id::from("tail")),
                value: ElementAccess{
                    value: Memory(Id::from("cons")).into(),
                    idx: 1
                }.into(),
            }.into(),
        ],
        "auto tail = std::get<1ULL>(cons);";
        "cons extraction"
    )]
    #[test_case(
        vec![
            Assignment {
                target: Memory(Id::from("n")).into(),
                value: ConstructorCall{
                    type_: Name::from("Nat"),
                    idx: 1,
                    data: None
                }.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("s")).into(),
                value: ConstructorCall{
                    type_: Name::from("Nat"),
                    idx: 0,
                    data: Some((
                        Name::from("Suc"),
                        Memory(Id::from("n")).into()
                    ))
                }.into(),
            }.into(),
        ],
        "auto n = Nat{std::integral_constant<std::size_t,1>()}; auto s = Nat{std::integral_constant<std::size_t,0>(), Suc{ensure_lazy(n)}};";
        "simple recursive type instantiation"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("bull"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::BOOL.into(),
                memory: Memory(Id::from("r"))
            }.into(),
            MatchStatement{
                expression: (Memory(Id::from("bull")).into(), UnionType(vec![Name::from("Twoo"), Name::from("Faws")]).into()),
                auxiliary_memory: Memory(Id::from("tmp")),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("r")).into(),
                                value: Expression::Value(Value::BuiltIn(Boolean{value: true}.into()).into()),
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("r")).into(),
                                value: Expression::Value(Value::BuiltIn(Boolean{value: false}.into()).into()),
                            }.into(),
                        ],
                    }
                ]
            }.into()
        ],
        "LazyT<Bool> r; WorkManager::await(bull); auto tmp = extract_lazy(bull); switch (tmp.tag) { case 0ULL: { r = ensure_lazy(Bool{true}); break; } case 1ULL: { r = ensure_lazy(Bool{false}); break; }}";
        "match statement no values"
    )]
    #[test_case(
        vec![
            Declaration {
                type_: AtomicTypeEnum::INT.into(),
                memory: Memory(Id::from("call"))
            }.into(),
            MatchStatement {
                auxiliary_memory: Memory(Id::from("tmp")),
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
                                            AtomicType(AtomicTypeEnum::INT).into(),
                                            AtomicType(AtomicTypeEnum::INT).into()
                                        ],
                                        Box::new(AtomicType(AtomicTypeEnum::BOOL).into()),
                                    ),
                                    args: vec![
                                        Memory(Id::from("x")).into(),
                                        Memory(Id::from("y")).into(),
                                    ]
                                }.into(),
                            }.into()
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Name::from("x"))),
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("call")).into(),
                                value: Value::from(Memory(Id::from("x"))).into(),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
        ],
        "LazyT<Int> call; auto tmp = extract_lazy(either); switch (tmp.tag) {case 0ULL: { LazyT<Left::type> x = reinterpret_cast<Left*>(&tmp.value)->value; call = ensure_lazy(Comparison_GE__BuiltIn(x,y)); break; } case 1ULL:{ LazyT<Right::type> x = reinterpret_cast<Right*>(&tmp.value)->value; call = ensure_lazy(x); break; }}";
        "match statement read values"
    )]
    #[test_case(
        vec![
            Declaration {
                type_: MachineType::NamedType(Name::from("Nat")),
                memory: Memory(Id::from("r"))
            }.into(),
            MatchStatement {
                auxiliary_memory: Memory(Id::from("nat_")),
                expression: (Memory(Id::from("nat")).into(), UnionType(vec![Name::from("Suc"), Name::from("Nil")])),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Name::from("s"))),
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("r")).into(),
                                value: Expression::Value(Memory(Name::from("s")).into())
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("r")).into(),
                                value: Expression::Value(Memory(Name::from("nil")).into()),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
        ],
        "LazyT<Nat> r; auto nat_ = extract_lazy(nat); switch (nat_.tag) { case 0ULL: { LazyT<Suc::type> s = reinterpret_cast<Suc*>(&nat_.value)->value; r = ensure_lazy(s); break; } case 1ULL: { r = ensure_lazy(nil); break; }}";
        "match statement recursive type"
    )]
    fn test_statements_emission(statements: Vec<Statement>, expected: &str) {
        let code = EMITTER.emit_statements(statements, HashSet::new());
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        FnDef {
            env: Vec::new(),
            name: Name::from("IdentityInt"),
            arguments: vec![(Memory(Id::from("x")), AtomicType(AtomicTypeEnum::INT).into())],
            statements: Vec::new(),
            ret: (Memory(Id::from("x")).into(), AtomicType(AtomicTypeEnum::INT).into()),
            size_bounds: (1, 1),
            is_recursive: false
        },
        "struct IdentityInt : TypedClosureI<Empty, Int, Int> { using TypedClosureI<Empty, Int, Int>::TypedClosureI; LazyT<Int> body(LazyT<Int> &x) override { return ensure_lazy(x); } constexpr std::size_t lower_size_bound() const override { return 1; }; constexpr std::size_t upper_size_bound() const override { return 1; }; constexpr bool is_recursive() const override { return false; }; static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) { return std::make_unique<IdentityInt>(args); } static inline FnT<Int,Int>G = std::make_shared<TypedClosureG<Empty,Int,Int>>(init);};";
        "identity int"
    )]
    #[test_case(
        FnDef {
            env: Vec::new(),
            name: Name::from("FourWayPlus"),
            arguments: vec![
                (Memory(Id::from("a")), AtomicType(AtomicTypeEnum::INT).into()),
                (Memory(Id::from("b")), AtomicType(AtomicTypeEnum::INT).into()),
                (Memory(Id::from("c")), AtomicType(AtomicTypeEnum::INT).into()),
                (Memory(Id::from("d")), AtomicType(AtomicTypeEnum::INT).into()),
            ],
            statements: vec![
                Assignment {
                    target: Memory(Id::from("res1")),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                AtomicType(AtomicTypeEnum::INT).into(),
                                AtomicType(AtomicTypeEnum::INT).into()
                            ],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                            Memory(Id::from("b")).into(),
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("res2")),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                AtomicType(AtomicTypeEnum::INT).into(),
                                AtomicType(AtomicTypeEnum::INT).into()
                            ],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ),
                        args: vec![
                            Memory(Id::from("c")).into(),
                            Memory(Id::from("d")).into()
                        ]
                    }.into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("res3")),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                AtomicType(AtomicTypeEnum::INT).into(),
                                AtomicType(AtomicTypeEnum::INT).into()
                            ],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ),
                        args: vec![
                            Memory(Id::from("res1")).into(),
                            Memory(Id::from("res2")).into(),
                        ]
                    }.into()
                }.into(),
            ],
            ret: (Memory(Id::from("res3")).into(), AtomicType(AtomicTypeEnum::INT).into()),
            size_bounds: (90, 90),
            is_recursive: false
        },
        "struct FourWayPlus : TypedClosureI<Empty, Int, Int, Int, Int, Int> { using TypedClosureI<Empty, Int, Int, Int, Int, Int>::TypedClosureI; LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b, LazyT<Int> &c, LazyT<Int> &d) override { auto res1 = Plus__BuiltIn(a, b); auto res2 = Plus__BuiltIn(c, d); auto res3 = Plus__BuiltIn(res1, res2); return ensure_lazy(res3); } constexpr std::size_t lower_size_bound() const override { return 90; }; constexpr std::size_t upper_size_bound() const override { return 90; }; constexpr bool is_recursive() const override { return false; }; static std::unique_ptr<TypedFnI<Int, Int, Int, Int, Int>> init(const ArgsT &args) { return std::make_unique<FourWayPlus>(args); } static inline FnT<Int,Int,Int,Int,Int>G = std::make_shared<TypedClosureG<Empty,Int,Int,Int,Int,Int>>(init);};";
        "four way plus"
    )]
    #[test_case(
        FnDef{
            env: vec![AtomicType(AtomicTypeEnum::INT).into()],
            name: Name::from("Adder"),
            arguments: vec![(Memory(Id::from("x")), AtomicType(AtomicTypeEnum::INT).into())],
            statements: vec![
                Assignment {
                    target: Memory(Id::from("y")),
                    value: ElementAccess{
                        idx: 0,
                        value: Memory(Id::from("env")).into()
                    }.into(),
                }.into(),
                Assignment {
                    target: Memory(Id::from("inner_res")),
                    value: FnCall{
                        fn_: BuiltIn::BuiltInFn(
                            Name::from("Plus__BuiltIn"),
                        ).into(),
                        fn_type: FnType(
                            vec![
                                AtomicType(AtomicTypeEnum::INT).into(),
                                AtomicType(AtomicTypeEnum::INT).into(),
                            ],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                            Memory(Id::from("y")).into(),
                        ]
                    }.into(),
                }.into(),
            ],
            ret: (Memory(Id::from("inner_res")).into(), AtomicType(AtomicTypeEnum::INT).into()),
            size_bounds: (50, 80),
            is_recursive: false
        },
        "struct Adder : TypedClosureI<TupleT<Int>, Int, Int> { using TypedClosureI<TupleT<Int>, Int, Int>::TypedClosureI; LazyT<Int> body(LazyT<Int> &x) override { auto y = load_env(std::get<0ULL>(env)); auto inner_res = Plus__BuiltIn(x, y); return ensure_lazy(inner_res); } constexpr std::size_t lower_size_bound() const override { return 50; }; constexpr std::size_t upper_size_bound() const override { return 80; }; constexpr bool is_recursive() const override { return false; }; static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args, const EnvT &env) { return std::make_unique<Adder>(args, env); }};";
        "adder closure"
    )]
    #[test_case(
        FnDef{
            env: vec![AtomicType(AtomicTypeEnum::INT).into()],
            name: Name::from("Apply"),
            arguments: vec![
                (Memory(Id::from("f")), FnType(vec![AtomicType(AtomicTypeEnum::INT).into()], Box::new(AtomicType(AtomicTypeEnum::INT).into())).into()),
                (Memory(Id::from("x")), AtomicType(AtomicTypeEnum::INT).into()),
            ],
            statements: vec![
                Assignment {
                    target: Memory(Id::from("y")),
                    value: FnCall{
                        fn_: Memory(Id::from("f")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicType(AtomicTypeEnum::INT).into(),
                            ],
                            Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                        ]
                    }.into(),
                }.into(),
            ],
            ret: (Memory(Id::from("y")).into(), AtomicType(AtomicTypeEnum::INT).into()),
            size_bounds: (150, 150),
            is_recursive: true
        },
        "struct Apply : TypedClosureI<TupleT<Int>,Int,FnT<Int,Int>,Int>{ using TypedClosureI<TupleT<Int>,Int,FnT<Int,Int>,Int>::TypedClosureI; LazyT<Int> body(LazyT<FnT<Int,Int>> &f, LazyT<Int> &x) override { auto y = fn_call(extract_lazy(f),x); return ensure_lazy(y);} constexpr std::size_t lower_size_bound() const override {return 150;}; constexpr std::size_t upper_size_bound() const override {return 150;}; constexpr bool is_recursive() const override {return true;}; static std::unique_ptr<TypedFnI<Int,FnT<Int,Int>,Int>> init(const ArgsT&args,const EnvT&env) {return std::make_unique<Apply>(args,env);}};";
        "higher order fn"
    )]
    fn test_fn_def_emission(fn_def: FnDef, expected: &str) {
        let code = EMITTER.emit_fn_def(fn_def);
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
                    env: Vec::new(),
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
                                        AtomicType(AtomicTypeEnum::INT).into(),
                                        AtomicType(AtomicTypeEnum::INT).into()
                                    ],
                                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                                ),
                                args: vec![
                                    Memory(Id::from("x")).into(),
                                    Memory(Id::from("y")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    ret: (Memory(Id::from("call")).into(), AtomicType(AtomicTypeEnum::INT).into()),
                    size_bounds: (50, 50),
                    is_recursive: false
                },
                FnDef {
                    env: Vec::new(),
                    name: Name::from("PreMain"),
                    arguments: Vec::new(),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("x")).into(),
                            value: Expression::Value(Value::BuiltIn(Integer{value: 9}.into())),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("y")).into(),
                            value: Expression::Value(Value::BuiltIn(Integer{value: 5}.into())),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("main")),
                            value: FnCall{
                                fn_: Memory(
                                    Id::from("Main"),
                                ).into(),
                                fn_type: FnType(
                                    Vec::new(),
                                    Box::new(AtomicType(AtomicTypeEnum::INT).into()),
                                ),
                                args: Vec::new()
                            }.into()
                        }.into(),
                    ],
                    ret: (Memory(Id::from("main")).into(), AtomicType(AtomicTypeEnum::INT).into()),
                    size_bounds: (40, 60),
                    is_recursive: false
                }
            ],
        },
        "#include \"main/include.hpp\" struct Twoo; struct Faws; typedef VariantT<Twoo,Faws>Bull; struct Twoo {Empty value;}; struct Faws {Empty value;}; struct Main : TypedClosureI<Empty,Int> {using TypedClosureI<Empty,Int>::TypedClosureI; LazyT<Int> body() override { auto call = Plus__BuiltIn(x,y); return ensure_lazy(call);} constexpr std::size_t lower_size_bound() const override { return 50; }; constexpr std::size_t upper_size_bound() const override { return 50; }; constexpr bool is_recursive() const override { return false; }; static std::unique_ptr<TypedFnI<Int>> init(const ArgsT &args) {return std::make_unique<Main>(args);} static inline FnT<Int>G = std::make_shared<TypedClosureG<Empty,Int>>(init);}; struct PreMain : TypedClosureI<Empty,Int> {using TypedClosureI<Empty,Int>::TypedClosureI; LazyT<Int> body() override { auto x = Int{9LL}; auto y = Int{5LL}; auto main = fn_call(extract_lazy(Main)); return ensure_lazy(main); } constexpr std::size_t lower_size_bound() const override { return 40; }; constexpr std::size_t upper_size_bound() const override { return 60; }; constexpr bool is_recursive() const override { return false; }; static std::unique_ptr<TypedFnI<Int>>init(const ArgsT&args) {return std::make_unique<PreMain>(args);} static inline FnT<Int> G = std::make_shared<TypedClosureG<Empty,Int>>(init);};";
        "main program"
    )]
    fn test_program_emission(program: Program, expected: &str) {
        let code = Emitter::emit(program);
        let expected_code = Code::from(expected);
        assert_eq_code(code, expected_code);
    }
}
