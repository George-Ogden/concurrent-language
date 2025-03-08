use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use crate::{
    code_vector::CodeVectorCalculator, weakener::Weakener, Assignment, Await, BuiltIn,
    ClosureInstantiation, CompilationArgs, ConstructorCall, Declaration, ElementAccess, Expression,
    FnCall, FnDef, FnType, Id, IfStatement, MachineType, MatchBranch, MatchStatement, Memory, Name,
    Program, Statement, TupleExpression, TupleType, TypeDef, UnionType, Value,
};
use itertools::Itertools;
use lowering::*;
use once_cell::sync::Lazy;

const OPERATOR_NAMES: Lazy<HashMap<Id, Id>> = Lazy::new(|| {
    HashMap::from_iter(
        [
            ("+", "Plus__BuiltIn"),
            ("-", "Minus__BuiltIn"),
            ("*", "Multiply__BuiltIn"),
            ("/", "Divide__BuiltIn"),
            ("**", "Exponentiate__BuiltIn"),
            ("%", "Modulo__BuiltIn"),
            ("<<", "Left_Shift__BuiltIn"),
            (">>", "Right_Shift__BuiltIn"),
            ("<=>", "Spaceship__BuiltIn"),
            ("&", "Bitwise_And__BuiltIn"),
            ("|", "Bitwise_Or__BuiltIn"),
            ("^", "Bitwise_Xor__BuiltIn"),
            ("++", "Increment__BuiltIn"),
            ("--", "Decrement__BuiltIn"),
            ("<", "Comparison_LT__BuiltIn"),
            ("<=", "Comparison_LE__BuiltIn"),
            (">", "Comparison_GT__BuiltIn"),
            (">=", "Comparison_GE__BuiltIn"),
            ("==", "Comparison_EQ__BuiltIn"),
            ("!=", "Comparison_NE__BuiltIn"),
            ("!", "Negation__BuiltIn"),
        ]
        .into_iter()
        .map(|(op, name)| (Id::from(op), Id::from(name))),
    )
});

type ReferenceNames = HashMap<*mut IntermediateType, MachineType>;
type MemoryIds = HashMap<Location, Memory>;
type TypeLookup = HashMap<IntermediateUnionType, UnionType>;
type FnDefs = Vec<FnDef>;

pub struct Compiler {
    reference_names: ReferenceNames,
    memory_ids: MemoryIds,
    type_lookup: TypeLookup,
    fn_defs: FnDefs,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            reference_names: ReferenceNames::new(),
            memory_ids: MemoryIds::new(),
            type_lookup: TypeLookup::new(),
            fn_defs: FnDefs::new(),
        }
    }

    fn compile_type(&self, type_: &IntermediateType) -> MachineType {
        match type_ {
            IntermediateType::AtomicType(atomic_type) => {
                let atomic_type_enum = atomic_type.0;
                atomic_type_enum.clone().into()
            }
            IntermediateType::IntermediateTupleType(IntermediateTupleType(types)) => {
                TupleType(self.compile_types(types)).into()
            }
            IntermediateType::IntermediateFnType(IntermediateFnType(arg_types, ret_type)) => {
                FnType(
                    self.compile_types(arg_types),
                    Box::new(self.compile_type(&*ret_type)),
                )
                .into()
            }
            IntermediateType::IntermediateUnionType(union_type) => {
                self.type_lookup[union_type].clone().into()
            }
            IntermediateType::Reference(reference) => {
                match self.reference_names.get(&reference.as_ptr()) {
                    Some(type_) => type_.clone(),
                    None => self.compile_type(&reference.borrow().clone()),
                }
            }
        }
    }
    fn compile_types(&self, types: &Vec<IntermediateType>) -> Vec<MachineType> {
        types.iter().map(|type_| self.compile_type(type_)).collect()
    }
    fn compile_type_defs(&mut self, types: Vec<Rc<RefCell<IntermediateType>>>) -> Vec<TypeDef> {
        let types = types
            .into_iter()
            .filter_map(|type_| {
                let IntermediateType::IntermediateUnionType(union_type) = type_.borrow().clone()
                else {
                    return None;
                };
                Some((type_.as_ptr(), union_type))
            })
            .collect_vec();
        for (i, (ptr, _)) in types.iter().enumerate() {
            self.reference_names
                .insert(*ptr, MachineType::NamedType(format!("T{i}")));
        }
        let machine_types = types
            .iter()
            .enumerate()
            .map(|(i, (_, IntermediateUnionType(types)))| {
                let names = types
                    .iter()
                    .enumerate()
                    .map(|(j, _)| format!("T{i}C{j}"))
                    .collect_vec();
                let intermediate_type = UnionType(names);
                self.type_lookup.insert(
                    IntermediateUnionType(types.clone()),
                    intermediate_type.clone(),
                );
                (format!("T{i}"), intermediate_type)
            })
            .collect_vec();
        types
            .into_iter()
            .zip(machine_types.into_iter())
            .map(
                |((_, IntermediateUnionType(types)), (type_name, machine_type))| {
                    let UnionType(ctor_names) = machine_type;
                    let constructors = types
                        .into_iter()
                        .zip(ctor_names.into_iter())
                        .map(|(type_, name)| {
                            (name, type_.as_ref().map(|type_| self.compile_type(type_)))
                        })
                        .collect_vec();
                    TypeDef {
                        name: type_name,
                        constructors,
                    }
                },
            )
            .collect_vec()
    }

    fn next_memory_address(&self) -> Memory {
        Memory(format!("m{}", self.memory_ids.len()))
    }
    fn compile_location(&mut self, location: &Location) -> Memory {
        if !self.memory_ids.contains_key(location) {
            self.memory_ids
                .insert(location.clone(), self.next_memory_address());
        }
        self.memory_ids[location].clone().into()
    }
    fn compile_memory(&mut self, memory: &IntermediateMemory) -> Memory {
        self.compile_location(&memory.location)
    }
    fn compile_arg(&mut self, arg: &IntermediateArg) -> Memory {
        self.compile_location(&arg.location)
    }
    fn new_memory_location(&mut self) -> Memory {
        let mut boxes: Vec<Location> = Vec::new();
        while match boxes.last() {
            None => true,
            Some(x) => self.memory_ids.contains_key(&x),
        } {
            boxes.push(Location::new());
        }
        let last = boxes.last().unwrap();
        let memory = self.next_memory_address();
        self.memory_ids.insert(last.clone(), memory.clone());
        memory
    }
    fn next_fn_name(&self) -> Name {
        format!("F{}", self.fn_defs.len())
    }
    fn compile_value(&mut self, value: IntermediateValue) -> Value {
        match &value {
            IntermediateValue::IntermediateArg(arg) => self.compile_arg(arg).into(),
            IntermediateValue::IntermediateMemory(memory) => self.compile_memory(memory).into(),
            IntermediateValue::IntermediateBuiltIn(built_in) => Value::from(match built_in {
                IntermediateBuiltIn::Boolean(boolean) => BuiltIn::from(boolean.clone()),
                IntermediateBuiltIn::Integer(integer) => BuiltIn::from(integer.clone()),
                IntermediateBuiltIn::BuiltInFn(BuiltInFn(name, _)) => {
                    BuiltIn::BuiltInFn(OPERATOR_NAMES[name].clone()).into()
                }
            }),
        }
    }
    fn compile_values(&mut self, values: Vec<IntermediateValue>) -> Vec<Value> {
        values
            .into_iter()
            .map(|value| self.compile_value(value))
            .collect()
    }
    fn compile_expression(
        &mut self,
        expression: IntermediateExpression,
    ) -> (Vec<Statement>, Expression, Vec<Declaration>) {
        match expression {
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                let values = self.compile_values(values);
                (Vec::new(), TupleExpression(values).into(), Vec::new())
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                let value = self.compile_value(value);
                (Vec::new(), ElementAccess { value, idx }.into(), Vec::new())
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                let MachineType::FnType(fn_type) = self.compile_type(&fn_.type_()) else {
                    panic!("Function has non-function type.")
                };
                let fn_value = self.compile_value(fn_);
                let args_values = self.compile_values(args);
                (
                    if let Value::Memory(mem) = &fn_value {
                        vec![Await(vec![mem.clone()]).into()]
                    } else {
                        Vec::new()
                    },
                    FnCall {
                        fn_: fn_value,
                        fn_type,
                        args: args_values,
                    }
                    .into(),
                    Vec::new(),
                )
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => {
                let value = data.map(|data| self.compile_value(data));
                (
                    Vec::new(),
                    ConstructorCall {
                        idx,
                        data: value.map(|value| {
                            let MachineType::UnionType(UnionType(variants)) =
                                self.compile_type(&type_.into())
                            else {
                                panic!("Did not compile union type into union type.")
                            };
                            (variants[idx].clone(), value)
                        }),
                    }
                    .into(),
                    Vec::new(),
                )
            }
            IntermediateExpression::IntermediateValue(value) => {
                let value = self.compile_value(value);
                (Vec::new(), value.into(), Vec::new())
            }
            IntermediateExpression::ILambda(lambda) => {
                let (statements, closure_inst) = self.compile_lambda(lambda);
                (statements, closure_inst.into(), Vec::new())
            }
            IntermediateExpression::IIf(if_) => {
                let (statements, value, allocations) = self.compile_if(if_);
                (statements, value.into(), allocations)
            }
            IntermediateExpression::IMatch(match_) => {
                let (statements, value, allocations) = self.compile_match(match_);
                (statements, value.into(), allocations)
            }
        }
    }
    fn compile_if(&mut self, if_: IIf) -> (Vec<Statement>, Value, Vec<Declaration>) {
        let IIf {
            condition,
            branches: (true_block, false_block),
        } = if_;
        let condition = self.compile_value(condition);
        let mut statements = if let Value::Memory(mem) = &condition {
            vec![Await(vec![mem.clone()]).into()]
        } else {
            Vec::new()
        };
        let target = IntermediateMemory::from(true_block.type_());
        let memory = self.compile_memory(&target);
        statements.push(
            Declaration {
                memory: memory.clone(),
                type_: self.compile_type(&target.type_()),
            }
            .into(),
        );
        let (mut true_statements, true_value, true_allocations) = self.compile_block(true_block);
        true_statements.push(
            Assignment {
                target: memory.clone(),
                value: true_value.into(),
            }
            .into(),
        );
        let (mut false_statements, false_value, false_allocations) =
            self.compile_block(false_block);
        false_statements.push(
            Assignment {
                target: memory.clone(),
                value: false_value.into(),
            }
            .into(),
        );
        statements.push(
            IfStatement {
                condition,
                branches: (true_statements, false_statements),
            }
            .into(),
        );
        let mut allocations = true_allocations;
        allocations.extend(false_allocations);
        (statements, memory.into(), allocations)
    }
    fn compile_match(&mut self, match_: IMatch) -> (Vec<Statement>, Value, Vec<Declaration>) {
        let IMatch { subject, branches } = match_;
        let type_ = subject.type_();
        let MachineType::UnionType(union_type) = self.compile_type(&type_) else {
            panic!("Match expression subject has non-union type.")
        };
        let subject = self.compile_value(subject);
        let mut statements = if let Value::Memory(mem) = &subject {
            vec![Await(vec![mem.clone()]).into()]
        } else {
            Vec::new()
        };
        let result = IntermediateMemory::from(branches[0].block.type_());
        let memory = self.compile_memory(&result);
        statements.push(
            Declaration {
                memory: memory.clone(),
                type_: self.compile_type(&result.type_()),
            }
            .into(),
        );
        let (branches, allocations): (Vec<_>, Vec<_>) = branches
            .into_iter()
            .map(|IntermediateMatchBranch { target, block }| {
                let (mut statements, value, allocations) = self.compile_block(block);
                statements.push(
                    Assignment {
                        target: memory.clone(),
                        value: value.into(),
                    }
                    .into(),
                );
                let branch = MatchBranch {
                    target: target.map(|arg| self.compile_arg(&arg)),
                    statements,
                };
                (branch, allocations)
            })
            .unzip();
        statements.push(
            MatchStatement {
                expression: (subject, union_type),
                branches,
                auxiliary_memory: self.new_memory_location(),
            }
            .into(),
        );
        let value = self.compile_memory(&result);
        (statements, value.into(), allocations.concat())
    }
    fn compile_statement(
        &mut self,
        statement: IntermediateStatement,
    ) -> (Vec<Statement>, Vec<Declaration>) {
        match statement {
            IntermediateStatement::IntermediateAssignment(memory) => {
                self.compile_assignment(memory)
            }
        }
    }
    fn compile_assignment(
        &mut self,
        assignment: IntermediateAssignment,
    ) -> (Vec<Statement>, Vec<Declaration>) {
        let IntermediateAssignment {
            expression,
            location,
        } = assignment;
        let type_ = self.compile_type(&expression.type_());
        let (mut statements, value, mut allocations) = self.compile_expression(expression);
        let memory = self.compile_location(&location);
        let assignment = Assignment {
            target: memory.clone(),
            value: value.clone(),
        };
        let declaration = Declaration {
            memory: memory.clone().into(),
            type_,
        };
        if matches!(
            &value,
            Expression::FnCall(FnCall {
                fn_: Value::Memory(_),
                fn_type: _,
                args: _
            })
        ) {
            allocations.push(declaration.into());
            statements.push(assignment.into());
        } else {
            statements.push(declaration.into());
            statements.push(assignment.into());
        }
        (statements, allocations)
    }
    fn compile_statements(
        &mut self,
        statements: Vec<IntermediateStatement>,
    ) -> (Vec<Statement>, Vec<Declaration>) {
        let (statements, allocations): (Vec<_>, Vec<_>) = statements
            .into_iter()
            .map(|statement| self.compile_statement(statement))
            .unzip();
        (statements.concat(), allocations.concat())
    }
    fn compile_block(&mut self, block: IBlock) -> (Vec<Statement>, Value, Vec<Declaration>) {
        let (statements, declarations): (Vec<_>, Vec<_>) = block
            .statements
            .into_iter()
            .map(|statement| self.compile_statement(statement))
            .unzip();
        let value = self.compile_value(block.ret);
        (statements.concat(), value, declarations.concat())
    }
    fn replace_open_vars(&mut self, fn_def: &mut ILambda) -> Vec<(IntermediateValue, Location)> {
        let open_vars = fn_def.find_open_vars();
        let new_locations = open_vars
            .iter()
            .map(|val| IntermediateMemory::from(val.type_().clone()))
            .collect_vec();
        let substitution = open_vars
            .iter()
            .zip(new_locations.iter())
            .map(|(var, mem)| (var.location().unwrap().clone(), mem.location.clone()))
            .collect::<HashMap<_, _>>();
        fn_def.substitute(&substitution);
        open_vars
            .iter()
            .zip(new_locations.iter())
            .map(|(val, mem)| (val.clone().into(), mem.location.clone()))
            .collect()
    }
    fn closure_prefix(&mut self, env_types: &Vec<(Location, MachineType)>) -> Vec<Statement> {
        env_types
            .iter()
            .enumerate()
            .flat_map(|(i, (location, type_))| {
                let memory = self.compile_location(location);
                vec![
                    Declaration {
                        memory: memory.clone(),
                        type_: type_.clone(),
                    }
                    .into(),
                    Assignment {
                        target: memory,
                        value: ElementAccess {
                            idx: i,
                            value: Memory(Id::from("env")).into(),
                        }
                        .into(),
                    }
                    .into(),
                ]
            })
            .collect_vec()
    }
    fn compile_lambda(&mut self, mut lambda: ILambda) -> (Vec<Statement>, ClosureInstantiation) {
        let env_mapping = self.replace_open_vars(&mut lambda);
        let env_types = env_mapping
            .iter()
            .map(|(value, location)| (location.clone(), self.compile_type(&value.type_())))
            .collect_vec();

        let ILambda {
            args,
            block:
                IBlock {
                    statements,
                    ret: return_value,
                },
        } = lambda;
        let args = args
            .into_iter()
            .map(|arg| (self.compile_arg(&arg), self.compile_type(&arg.type_())))
            .collect_vec();
        let mut prefix = self.closure_prefix(&env_types);
        let (mut statements, allocations) = self.compile_statements(statements);
        prefix.extend(statements);
        statements = prefix;
        let ret_type = self.compile_type(&return_value.type_());
        let ret_val = self.compile_value(return_value);
        let name = self.next_fn_name();
        let env_types = env_types.into_iter().map(|(_, type_)| type_).collect_vec();
        self.fn_defs.push(FnDef {
            name: name.clone(),
            arguments: args,
            statements,
            ret: (ret_val, ret_type),
            env: env_types.clone(),
            allocations,
        });

        if env_mapping.len() > 0 {
            let tuple_mem = self.new_memory_location();
            let values = env_mapping
                .into_iter()
                .map(|(value, _)| self.compile_value(value))
                .collect();
            let statements = vec![
                Declaration {
                    memory: tuple_mem.clone(),
                    type_: TupleType(env_types).into(),
                }
                .into(),
                Assignment {
                    target: tuple_mem.clone(),
                    value: TupleExpression(values).into(),
                }
                .into(),
            ];
            (
                statements,
                ClosureInstantiation {
                    name,
                    env: Some(tuple_mem.into()),
                },
            )
        } else {
            (Vec::new(), ClosureInstantiation { name, env: None })
        }
    }
    fn compile_program(&mut self, program: IntermediateProgram) -> Program {
        let IntermediateProgram { main, types } = program;
        let type_defs = self.compile_type_defs(types);
        let (statements, _) = self.compile_lambda(main);
        assert_eq!(statements.len(), 0);
        self.fn_defs.last_mut().unwrap().name = Name::from("Main");
        let program = Program {
            fn_defs: self.fn_defs.clone(),
            type_defs,
        };
        Weakener::weaken(program)
    }
    pub fn compile(program: IntermediateProgram, args: CompilationArgs) -> Program {
        let mut compiler = Compiler::new();
        if let Some(filename) = args.export_vector_file {
            Self::export_vector(&program, filename).expect("Failed to save program")
        };
        compiler.compile_program(program)
    }
    fn export_vector(program: &IntermediateProgram, filename: String) -> Result<(), String> {
        let vector = CodeVectorCalculator::lambda_vector(&program.main);
        vector?
            .save(&Path::new(&filename))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {

    use std::{fs, path::PathBuf};

    use super::*;

    use lowering::{Boolean, Integer};
    use rstest::{fixture, rstest};
    use tempfile::TempDir;
    use test_case::test_case;

    #[test_case(
        Vec::new(),
        Vec::new();
        "empty type defs"
    )]
    #[test_case(
        vec![
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![None,None,]
            ).into())),
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![Some(AtomicTypeEnum::INT.into()),None,]
            ).into()))
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), None),
                    (Name::from("T0C1"), None),
                ]
            },
            TypeDef {
                name: Name::from("T1"),
                constructors: vec![
                    (Name::from("T1C0"), Some(AtomicTypeEnum::INT.into())),
                    (Name::from("T1C1"), None),
                ]
            },
        ];
        "non-recursive union types"
    )]
    #[test_case(
        vec![
            {
                let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
                let recursive_type = IntermediateUnionType(
                    vec![
                        Some(IntermediateType::Reference(reference.clone())),
                        None
                    ]
                ).into();
                *reference.borrow_mut() = recursive_type;
                reference
            },
            {
                let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
                let recursive_type = IntermediateUnionType(
                    vec![
                        Some(IntermediateTupleType(vec![IntermediateType::Reference(reference.clone()), AtomicTypeEnum::INT.into()]).into()),
                        None
                    ]
                ).into();
                *reference.borrow_mut() = recursive_type;
                reference
            },
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), Some(MachineType::NamedType(Name::from("T0")))),
                    (Name::from("T0C1"), None),
                ]
            },
            TypeDef {
                name: Name::from("T1"),
                constructors: vec![
                    (Name::from("T1C0"), Some(TupleType(vec![
                        MachineType::NamedType(Name::from("T1")),
                        AtomicTypeEnum::INT.into()
                    ]).into())),
                    (Name::from("T1C1"), None),
                ]
            },
        ];
        "recursive union types"
    )]
    #[test_case(
        vec![
            Rc::new(RefCell::new(IntermediateType::Reference(
                Rc::new(RefCell::new(
                    IntermediateTupleType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()]
                    ).into()
                ))
            ))),
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into()),]
            ).into()))
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), Some(AtomicTypeEnum::INT.into())),
                    (Name::from("T0C1"), Some(AtomicTypeEnum::BOOL.into())),
                ]
            },
        ];
        "mixed types"
    )]
    fn test_compile_type_defs(
        type_defs: Vec<Rc<RefCell<IntermediateType>>>,
        expected_type_defs: Vec<TypeDef>,
    ) {
        let mut compiler = Compiler::new();
        assert_eq!(compiler.compile_type_defs(type_defs), expected_type_defs)
    }

    #[test_case(
        IntermediateBuiltIn::from(Integer{value: 4}).into(),
        BuiltIn::from(Integer{value: 4}).into();
        "integer"
    )]
    #[test_case(
        IntermediateBuiltIn::from(Boolean{value: true}).into(),
        BuiltIn::from(Boolean{value: true}).into();
        "boolean"
    )]
    #[test_case(
        BuiltInFn(
            Name::from("=="),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::BOOL.into())
            ).into()
        ).into(),
        BuiltIn::BuiltInFn(
            Name::from("Comparison_EQ__BuiltIn"),
        ).into();
        "built-in fn"
    )]
    #[test_case(
        IntermediateValue::IntermediateMemory(
            IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT))
        ),
        Memory(
            Id::from("m0")
        ).into();
        "memory"
    )]
    #[test_case(
        IntermediateValue::IntermediateArg(
            IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))
        ),
        Memory(
            Id::from("m0")
        ).into();
        "argument"
    )]
    fn test_compile_values(value: IntermediateValue, expected_value: Value) {
        let mut compiler = Compiler::new();
        let compiled_value = compiler.compile_value(value);
        assert_eq!(compiled_value, expected_value);
    }
    #[test]
    fn test_compile_multiple_memory_locations() {
        let locations = vec![Location::new(), Location::new(), Location::new()];
        let mut compiler = Compiler::new();
        let value_0 = compiler.compile_location(&locations[0].clone());
        let value_1 = compiler.compile_location(&locations[1].clone());
        let value_2 = compiler.compile_location(&locations[2].clone());
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(value_0, compiler.compile_location(&locations[0].clone()));
        assert_eq!(value_1, compiler.compile_location(&locations[1].clone()));
        assert_eq!(value_2, compiler.compile_location(&locations[2].clone()));
    }
    #[test]
    fn test_compile_arguments() {
        let types: Vec<IntermediateType> = vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
            AtomicTypeEnum::INT.into(),
        ];
        let mut compiler = Compiler::new();

        let args = types
            .into_iter()
            .map(|type_| IntermediateArg::from(type_))
            .collect_vec();
        let value_0 = compiler.compile_value(args[0].clone().into());
        let value_1 = compiler.compile_value(args[1].clone().into());
        let value_2 = compiler.compile_value(args[2].clone().into());
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(value_0, compiler.compile_value(args[0].clone().into()));
        assert_eq!(value_1, compiler.compile_value(args[1].clone().into()));
        assert_eq!(value_2, compiler.compile_value(args[2].clone().into()));
    }

    #[test_case(
        IntermediateTupleExpression(Vec::new()).into(),
        (
            Vec::new(),
            TupleExpression(Vec::new()).into()
        );
        "empty expression"
    )]
    #[test_case(
        IntermediateTupleExpression(
            vec![
                IntermediateBuiltIn::from(Integer{value: 5}).into(),
                IntermediateBuiltIn::from(Boolean{value: true}).into(),
            ]
        ).into(),
        (
            Vec::new(),
            TupleExpression(vec![
                BuiltIn::from(Integer{value: 5}).into(),
                BuiltIn::from(Boolean{value: true}).into(),
            ]).into()
        );
        "tuple expression"
    )]
    #[test_case(
        IntermediateTupleExpression(
            vec![
                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into()
            ]
        ).into(),
        (
            Vec::new(),
            TupleExpression(vec![
                Memory(Id::from("m0")).into()
            ]).into()
        );
        "tuple expression with argument"
    )]
    #[test_case(
        IntermediateElementAccess{
            value: IntermediateArg::from(IntermediateType::from(
                IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]))
            ).into(),
            idx: 1
        }.into(),
        (
            Vec::new(),
            ElementAccess{
                value: Memory(Id::from("m0")).into(),
                idx: 1
            }.into()
        );
        "element access"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: BuiltInFn(
                Name::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            ).into(),
            args: vec![IntermediateBuiltIn::from(Integer{value: 7}).into()]
        }.into(),
        (
            Vec::new(),
            FnCall{
                args: vec![BuiltIn::from(Integer{value: 7}).into()],
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Increment__BuiltIn"),
                ).into(),
                fn_type: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            }.into()
        );
        "built-in fn call"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into())
                ))
            ).into(),
            args: vec![
                IntermediateArg::from(
                    IntermediateType::from(
                        AtomicTypeEnum::INT
                    )
                ).into(),
            ]
        }.into(),
        (
            vec![
                Await(vec![Memory(Id::from("m0"))]).into(),
            ],
            FnCall{
                args: vec![
                    Memory(Id::from("m1")).into(),
                ],
                fn_: Memory(Id::from("m0")).into(),
                fn_type: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into())
                )
            }.into()
        );
        "fn call higher-order call from args"
    )]
    #[test_case(
        IntermediateCtorCall{
            idx: 0,
            data: None,
            type_: IntermediateUnionType(vec![None,None])
        }.into(),
        (
            Vec::new(),
            ConstructorCall{
                idx: 0,
                data: None
            }.into()
        );
        "no data constructor call"
    )]
    fn test_compile_expressions(
        expression: IntermediateExpression,
        expected: (Vec<Statement>, Expression),
    ) {
        let mut compiler = Compiler::new();
        let (statements, expression, _) = compiler.compile_expression(expression);
        assert_eq!((statements, expression), expected);
    }
    #[test_case(
        {
            let type_ = IntermediateUnionType(vec![Some(AtomicTypeEnum::BOOL.into()),Some(AtomicTypeEnum::INT.into())]);
            (
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(IntermediateBuiltIn::from(Integer{value: 9}).into()),
                    type_: type_.clone()
                }.into(),
                Rc::new(RefCell::new(type_.into()))
            )
        },
        (
            Vec::new(),
            ConstructorCall{
                idx: 1,
                data: Some((Name::from("T0C1"), BuiltIn::from(Integer{value: 9}).into()))
            }.into()
        );
        "data constructor call"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
            let union_type = IntermediateUnionType(vec![Some(IntermediateType::Reference(reference.clone())),None]);
            *reference.borrow_mut() = union_type.clone().into();
            (
                IntermediateCtorCall{
                    idx: 0,
                    data: Some(IntermediateMemory::from(IntermediateType::from(union_type.clone())).into()),
                    type_: union_type
                }.into(),
                reference
            )
        },
        (
            Vec::new(),
            ConstructorCall{
                idx: 0,
                data: Some((Name::from("T0C0"), Memory(Id::from("m0")).into()))
            }.into()
        );
        "recursive constructor call"
    )]
    fn test_compile_constructors(
        constructor_type: (IntermediateCtorCall, Rc<RefCell<IntermediateType>>),
        expected: (Vec<Statement>, Expression),
    ) {
        let (constructor, type_) = constructor_type;
        let mut compiler = Compiler::new();
        compiler.compile_type_defs(vec![type_]);
        let (statements, expression, _) = compiler.compile_expression(constructor.into());
        assert_eq!((statements, expression), expected);
    }

    #[test_case(
        vec![
            IntermediateAssignment{
                expression:
                    IntermediateTupleExpression(vec![
                        IntermediateBuiltIn::from(Integer{value: 5}).into(),
                        IntermediateBuiltIn::from(Boolean{value: false}).into(),
                    ]).into()
                ,
                location: Location::new()
            }.into()
        ],
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: TupleExpression(vec![
                    BuiltIn::from(Integer{value: 5}).into(),
                    BuiltIn::from(Boolean{value: false}).into(),
                ]).into(),
            }.into()
        ];
        "tuple expression assignment"
    )]
    #[test_case(
        vec![
            IntermediateAssignment{
                expression:
                    IntermediateElementAccess{
                        idx: 1,
                        value: IntermediateArg::from(
                            IntermediateType::from(
                                IntermediateTupleType(vec![
                                    AtomicTypeEnum::INT.into(),
                                    AtomicTypeEnum::BOOL.into(),
                                ])
                            )
                        ).into()
                    }.into()
                ,
                location: Location::new()
            }.into()
        ],
        vec![
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::BOOL.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: ElementAccess{
                    idx: 1,
                    value: Memory(Id::from("m0")).into(),
                }.into(),
            }.into()
        ];
        "tuple access assignment"
    )]
    #[test_case(
        vec![
            IntermediateAssignment{
                expression: IntermediateFnCall{
                    fn_: BuiltInFn(
                        Name::from("--"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                    args: vec![
                        IntermediateBuiltIn::from(Integer{value: 11}).into()
                    ]
                }.into(),
                location: Location::new()
            }.into()
        ],
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: FnCall{
                    fn_: BuiltIn::BuiltInFn(
                        Name::from("Decrement__BuiltIn"),
                    ).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ),
                    args: vec![BuiltIn::from(Integer{value: 11}).into()]
                }.into(),
            }.into(),
        ];
        "built-in fn call"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            vec![
                IntermediateAssignment {
                    location: location.clone(),
                    expression: IIf{
                        condition: arg.into(),
                        branches: (
                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into(),
                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into(),
                        )
                    }.into()
                }.into()
            ]
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            IfStatement {
                condition: Memory(Id::from("m0")).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 1}).into()
                            ),
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 0}).into()
                            ),
                        }.into(),
                    ],
                )
            }.into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m2")),
                value: Expression::Value(Memory(Id::from("m1")).into()),
            }.into(),
        ];
        "if statement awaited argument"
    )]
    #[test_case(
        {
            let location = Location::new();
            vec![
                IntermediateAssignment {
                    location: location.clone(),
                    expression: IIf{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                            IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: false})).into(),
                        )
                    }.into()
                }.into()
            ]
        },
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement {
                condition: BuiltIn::from(Boolean{value: true}).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Boolean{value: true}).into()
                            ),
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Boolean{value: false}).into()
                            ),
                        }.into(),
                    ],
                )
            }.into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Value(Memory(Id::from("m0")).into()),
            }.into(),
        ];
        "if statement value only"
    )]
    #[test_case(
        {
            let location = Location::new();
            let temp = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            vec![
                IntermediateAssignment {
                    location: location.clone(),
                    expression: IIf{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            (
                                vec![
                                    IntermediateAssignment {
                                        location: temp.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: IntermediateMemory{
                                                location: Location::new(),
                                                type_: IntermediateFnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ).into()
                                            }.into(),
                                            args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                        }.into()
                                    }.into()
                                ],
                                temp.into()
                            ).into(),
                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                        )
                    }.into()
                }.into()
            ]
        },
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            IfStatement {
                condition: BuiltIn::from(Boolean{value: true}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: FnCall{
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ),
                                args: vec![BuiltIn::from(Integer{value: 0}).into()]
                            }.into(),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(Memory(Id::from("m2")).into()),
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(BuiltIn::from(Integer{value: 0}).into()),
                        }.into(),
                    ],
                )
            }.into(),
            Declaration {
                memory: Memory(Id::from("m3")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: Expression::Value(Memory(Id::from("m0")).into()),
            }.into(),
        ];
        "if statement value and call"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            vec![
                IntermediateAssignment{
                    location: location,
                    expression: ILambda {
                        args: vec![arg.clone()],
                        block: IBlock {
                            statements: Vec::new(),
                            ret: arg.clone().into()
                        },
                    }.into()
                }.into(),
            ]
        },
        vec![
            Declaration {
                type_: FnType(
                    vec![
                        AtomicTypeEnum::BOOL.into(),
                    ],
                    Box::new(AtomicTypeEnum::BOOL.into())
                ).into(),
                memory: Memory(Id::from("m1")),
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: ClosureInstantiation{
                    name: Name::from("F0"),
                    env: None
                }.into(),
            }.into()
        ];
        "identity function"
    )]
    fn test_compile_statements(
        statements: Vec<IntermediateStatement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut compiler = Compiler::new();
        let (compiled_statements, _) = compiler.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }
    #[test_case(
        {
            let bull_type: IntermediateType = IntermediateUnionType(vec![None,None]).into();
            let arg: IntermediateArg = IntermediateType::from(bull_type.clone()).into();
            let location = Location::new();
            (
                vec![Rc::new(RefCell::new(bull_type))],
                vec![
                    IntermediateAssignment {
                        location: location.clone(),
                        expression: IMatch{
                            subject: arg.into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                },
                                IntermediateMatchBranch{
                                    target: None,
                                    block:IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                }
                            ]
                        }.into()
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            MatchStatement {
                auxiliary_memory: Memory(Id::from("m2")),
                expression: (
                    Memory(Id::from("m0")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    BuiltIn::from(Integer{value: 1}).into()
                                ),
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    BuiltIn::from(Integer{value: 0}).into()
                                ),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Declaration {
                memory: Memory(Id::from("m3")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: Expression::Value(Memory(Id::from("m1")).into()),
            }.into(),
        ];
        "match statement no targets"
    )]
    #[test_case(
        {
            let either_type: IntermediateType = IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into())]).into();
            let arg: IntermediateArg = IntermediateType::from(either_type.clone()).into();
            let target0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let target1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let memory = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let temp = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![Rc::new(RefCell::new(either_type))],
                vec![
                    IntermediateAssignment {
                        location: memory.location.clone(),
                        expression: IMatch {
                            subject: arg.into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(target0.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment {
                                                location: temp.location.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: BuiltInFn(
                                                        Name::from(">"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::BOOL.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        target0.into(),
                                                        IntermediateBuiltIn::from(Integer{value: 0}).into()
                                                    ]
                                                }.into()
                                            }.into()
                                        ],
                                        temp.clone().into()
                                    ).into(),
                                },
                                IntermediateMatchBranch{
                                    target: Some(target1.clone()),
                                    block:IntermediateValue::from(target1).into()
                                }
                            ]
                        }.into(),
                    }.into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![memory.clone().into(), IntermediateBuiltIn::from(Integer{value: 0}).into()]
                            ).into()
                    }.into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![memory.clone().into(), IntermediateBuiltIn::from(Integer{value: 1}).into()]
                            ).into()
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration{
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m0")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                auxiliary_memory: Memory(Id::from("m5")),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("m2"))),
                        statements: vec![
                            Declaration {
                                type_: AtomicTypeEnum::BOOL.into(),
                                memory: Memory(Id::from("m3"))
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m3")),
                                value: FnCall{
                                    fn_: BuiltIn::BuiltInFn(
                                        Name::from("Comparison_GT__BuiltIn"),
                                    ).into(),
                                    fn_type: FnType(
                                        vec![
                                            AtomicTypeEnum::INT.into(),
                                            AtomicTypeEnum::INT.into()
                                        ],
                                        Box::new(AtomicTypeEnum::BOOL.into())
                                    ),
                                    args: vec![
                                        Memory(Id::from("m2")).into(),
                                        BuiltIn::from(Integer{value: 0}).into(),
                                    ]
                                }.into(),
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    Memory(Id::from("m3")).into(),
                                ),
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("m4"))),
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    Memory(Id::from("m4")).into(),
                                ),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Declaration {
                type_: AtomicTypeEnum::BOOL.into(),
                memory: Memory(Id::from("m6"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m6")),
                value: Value::from(Memory(Id::from("m1"))).into()
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m7"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m7")),
                value: TupleExpression(
                    vec![
                        Memory(Id::from("m6")).into(),
                        BuiltIn::from(Integer{value: 0}).into()
                    ]
                ).into(),
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m8"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m8")),
                value: TupleExpression(
                    vec![
                        Memory(Id::from("m6")).into(),
                        BuiltIn::from(Integer{value: 1}).into()
                    ]
                ).into(),
            }.into(),
        ];
        "match statement with targets and use"
    )]
    fn test_compile_match_statements(
        args_types_statements: (
            Vec<Rc<RefCell<IntermediateType>>>,
            Vec<IntermediateStatement>,
        ),
        expected_statements: Vec<Statement>,
    ) {
        let (types, statements) = args_types_statements;
        let mut compiler = Compiler::new();
        compiler.compile_type_defs(types);
        let (compiled_statements, _) = compiler.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }

    #[test_case(
        {
            let arg0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let arg1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y_expression: IntermediateExpression = IntermediateFnCall{
                fn_: BuiltInFn(
                    Name::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                ).into(),
                args: vec![
                    arg0.clone().into(),
                    arg1.clone().into(),
                ]
            }.into();
            ILambda {
                args: vec![arg0.clone(), arg1.clone()],
                block: IBlock{
                    statements: vec![
                        IntermediateAssignment{
                            location: y.location.clone(),
                            expression: y_expression,
                        }.into()
                    ],
                    ret: y.into()
                }
            }
        },
        (
            Vec::new(),
            ClosureInstantiation{
                name: Name::from("F0"),
                env: None
            },
            FnDef{
                name: Name::from("F0"),
                arguments: vec![
                    (Memory(Id::from("m0")), AtomicTypeEnum::INT.into()),
                    (Memory(Id::from("m1")), AtomicTypeEnum::INT.into()),
                ],
                env: Vec::new(),
                statements: vec![
                    Declaration {
                        memory: Memory(Id::from("m2")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment{
                        target: Memory(Id::from("m2")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Plus__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    AtomicTypeEnum::INT.into(),
                                    AtomicTypeEnum::INT.into()
                                ],
                                Box::new(AtomicTypeEnum::INT.into())
                            ),
                            args: vec![
                                Memory(Id::from("m0")).into(),
                                Memory(Id::from("m1")).into(),
                            ]
                        }.into(),
                    }.into()
                ],
                ret: (
                    Memory(Id::from("m2")).into(),
                    AtomicTypeEnum::INT.into()
                ),
                allocations: Vec::new()
            }
        );
        "env-free closure"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z_expression: IntermediateExpression = IntermediateFnCall{
                fn_: BuiltInFn(
                    Name::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                ).into(),
                args: vec![
                    x.clone().into(),
                    y.clone().into()
                ]
            }.into();
            ILambda {
                args: Vec::new(),
                block: IBlock{
                    statements: vec![
                        IntermediateAssignment{
                            location: z.location.clone(),
                            expression: z_expression,
                        }.into()
                    ],
                    ret: z.into()
                },
            }
        },
        (
            vec![
                Declaration {
                    memory: Memory(Id::from("m3")),
                    type_: TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::INT.into()
                    ]).into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("m3")),
                    value: TupleExpression(vec![
                        Memory(Id::from("m4")).into(),
                        Memory(Id::from("m5")).into(),
                    ]).into()
                }.into(),
            ],
            ClosureInstantiation{
                name: Name::from("F0"),
                env: Some(Memory(Id::from("m3")).into())
            },
            FnDef{
                name: Name::from("F0"),
                arguments: Vec::new(),
                env: vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::INT.into()
                ].into(),
                statements: vec![
                    Declaration {
                        memory: Memory(Id::from("m0")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m0")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 0
                        }.into()
                    }.into(),
                    Declaration {
                        memory: Memory(Id::from("m1")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m1")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 1
                        }.into()
                    }.into(),
                    Declaration {
                        memory: Memory(Id::from("m2")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment{
                        target: Memory(Id::from("m2")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Plus__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    AtomicTypeEnum::INT.into(),
                                    AtomicTypeEnum::INT.into()
                                ],
                                Box::new(AtomicTypeEnum::INT.into())
                            ),
                            args: vec![
                                Memory(Id::from("m0")).into(),
                                Memory(Id::from("m1")).into(),
                            ]
                        }.into(),
                    }.into()
                ],
                ret: (
                    Memory(Id::from("m2")).into(),
                    AtomicTypeEnum::INT.into()
                ),
                allocations: Vec::new()
            }
        );
        "env closure"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z_expression: IntermediateExpression = IntermediateFnCall{
                fn_: BuiltInFn(
                    Name::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                ).into(),
                args: vec![
                    x.clone().into(),
                    y.clone().into()
                ]
            }.into();
            ILambda {
                args: vec![y.clone()],
                block: IBlock{
                    statements: vec![
                        IntermediateAssignment{
                            location: z.location.clone(),
                            expression: z_expression,
                        }.into()
                    ],
                    ret: z.into()
                },
            }
        },
        (
            vec![
                Declaration {
                    memory: Memory(Id::from("m3")),
                    type_: TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                    ]).into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("m3")),
                    value: TupleExpression(vec![
                        Memory(Id::from("m4")).into(),
                    ]).into()
                }.into(),
            ],
            ClosureInstantiation{
                name: Name::from("F0"),
                env: Some(Memory(Id::from("m3")).into())
            },
            FnDef{
                name: Name::from("F0"),
                arguments: vec![(Memory(Id::from("m0")), AtomicTypeEnum::INT.into())],
                env: vec![
                    AtomicTypeEnum::INT.into(),
                ].into(),
                statements: vec![
                    Declaration {
                        memory: Memory(Id::from("m1")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m1")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 0
                        }.into()
                    }.into(),
                    Declaration {
                        memory: Memory(Id::from("m2")),
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                    Assignment{
                        target: Memory(Id::from("m2")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Plus__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    AtomicTypeEnum::INT.into(),
                                    AtomicTypeEnum::INT.into()
                                ],
                                Box::new(AtomicTypeEnum::INT.into())
                            ),
                            args: vec![
                                Memory(Id::from("m1")).into(),
                                Memory(Id::from("m0")).into(),
                            ]
                        }.into(),
                    }.into()
                ],
                ret: (
                    Memory(Id::from("m2")).into(),
                    AtomicTypeEnum::INT.into()
                ),
                allocations: Vec::new()
            }
        );
        "env and argument"
    )]
    fn test_compile_fn_defs(
        fn_def: ILambda,
        expected: (Vec<Statement>, ClosureInstantiation, FnDef),
    ) {
        let (expected_statements, expected_value, expected_fn_def) = expected;

        let mut compiler = Compiler::new();
        let compiled = compiler.compile_lambda(fn_def);
        assert_eq!(compiled, (expected_statements, expected_value));
        let compiled_fn_def = &compiler.fn_defs[0];
        assert_eq!(compiled_fn_def, &expected_fn_def);
    }

    #[test_case(
        {
            let identity = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT),
            );
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            IntermediateProgram {
                main: ILambda{
                    args: Vec::new(),
                    block: IBlock{
                        ret: main_call.clone().into(),
                        statements: vec![
                            IntermediateAssignment{
                                location: identity.location.clone(),
                                expression: ILambda{
                                    args: vec![arg.clone()],
                                    block: IBlock {
                                        statements: Vec::new(),
                                        ret: arg.clone().into()
                                    },
                                }.into()
                            }.into(),
                            IntermediateAssignment{
                                location: main.location.clone(),
                                expression:
                                    ILambda{
                                        args: Vec::new(),
                                        block: IBlock{
                                            statements: vec![
                                                IntermediateAssignment{
                                                    location: y.location.clone(),
                                                    expression: IntermediateFnCall{
                                                        fn_: identity.clone().into(),
                                                        args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                                    }.into()
                                                }.into()
                                            ],
                                            ret: y.clone().into()
                                        },
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                location: main_call.location.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                            }.into(),
                        ],
                    },
                },
                types: Vec::new()
            }
        },
        Program {
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: vec![(Memory(Id::from("m0")), AtomicTypeEnum::INT.into())],
                    statements: Vec::new(),
                    ret: (Memory(Id::from("m0")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    allocations: Vec::new()
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into())
                            )
                            .into(),
                            memory: Memory(Id::from("m2")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: ElementAccess {
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into(),
                        }.into(),
                        Await(vec![Memory(Id::from("m2"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: FnCall {
                                fn_: Memory(Id::from("m2")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ),
                                args: vec![BuiltIn::from(Integer { value: 0 }).into()]
                            }.into(),
                        }.into()
                    ],
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ].into(),
                    allocations: vec![
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m3"))
                        }
                    ]
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into())
                            ).into(),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: ClosureInstantiation { name: Name::from("F0"), env: None }.into(),
                        }.into(),
                        Declaration {
                            type_: TupleType(
                                vec![
                                    FnType(
                                        vec![AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::INT.into())
                                    ).into()
                                ]
                            ).into(),
                            memory: Memory(Id::from("m4"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
                        }.into(),
                        Declaration {
                            type_: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                            memory: Memory(Id::from("m5"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m5")),
                            value: ClosureInstantiation {
                                name: Name::from("F1"),
                                env: Some(Memory(Id::from("m4")).into())
                            }.into(),
                        }.into(),
                        Await(vec![Memory(Id::from("m5"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m6")),
                            value: FnCall {
                                fn_: Memory(Id::from("m5")).into(),
                                fn_type: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                                args: Vec::new()
                            }.into(),
                        }.into()
                    ],
                    ret: (Memory(Id::from("m6")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    allocations: vec![
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m6"))
                        }
                    ]
                }
            ]
        };
        "identity call program"
    )]
    #[test_case(
        {
            let t1 = IntermediateMemory::from(
                IntermediateType::from(IntermediateTupleType(Vec::new()))
            );
            let t2 = IntermediateMemory::from(
                IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(Vec::new()).into()])),
            );
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(IntermediateTupleType(vec![IntermediateTupleType(Vec::new()).into()]).into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(Vec::new()).into()])),
            );
            IntermediateProgram {
                main: ILambda{
                    args: Vec::new(),
                    block: IBlock{
                        ret: main_call.clone().into(),
                        statements: vec![
                            IntermediateAssignment{
                                location: t1.location.clone(),
                                expression:
                                    IntermediateTupleExpression(Vec::new()).into()
                            }.into(),
                            IntermediateAssignment{
                                location: t2.location.clone(),
                                expression:
                                    IntermediateTupleExpression(vec![t1.clone().into()]).into()
                            }.into(),
                            IntermediateAssignment{
                                location: main.location.clone(),
                                expression:
                                    ILambda{
                                        args: Vec::new(),
                                        block: IBlock {
                                            statements: Vec::new(),
                                            ret: t2.clone().into()
                                        },
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                location: main_call.location.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                            }.into(),
                        ],
                    },
                },
                types: Vec::new()
            }
        },
        Program {
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: TupleType(vec![TupleType(Vec::new()).into()]).into(),
                            memory: Memory(Id::from("m2")),
                        }.into(),
                        Assignment{
                            target: Memory(Id::from("m2")),
                            value: ElementAccess{
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into()
                        }.into()
                    ],
                    ret: (Memory(Id::from("m2")).into(), TupleType(vec![TupleType(Vec::new()).into()]).into()),
                    env: vec![TupleType(vec![TupleType(Vec::new()).into()]).into()].into(),
                    allocations: Vec::new()
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: TupleType(Vec::new()).into(),
                            memory: Memory(Id::from("m0")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: TupleExpression(Vec::new()).into(),
                        }.into(),
                        Declaration {
                            type_: TupleType(vec![TupleType(Vec::new()).into()]).into(),
                            memory: Memory(Id::from("m1")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: TupleExpression(vec![Memory(Id::from("m0")).into()]).into(),
                        }.into(),
                        Declaration {
                            type_: TupleType(vec![TupleType(vec![TupleType(Vec::new()).into()]).into()]).into(),
                            memory: Memory(Id::from("m3")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
                        }.into(),
                        Declaration {
                            type_: FnType(Vec::new(), Box::new(TupleType(vec![TupleType(Vec::new()).into()]).into())).into(),
                            memory: Memory(Id::from("m4")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: ClosureInstantiation{
                                name: Name::from("F0"),
                                env: Some(Memory(Id::from("m3")).into())
                            }.into()
                        }.into(),
                        Await(vec![Memory(Id::from("m4"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m5")),
                            value: FnCall{
                                fn_type: FnType(
                                    Vec::new(),
                                    Box::new(TupleType(vec![TupleType(Vec::new()).into()]).into())
                                ).into(),
                                fn_: Memory(Id::from("m4")).into(),
                                args: Vec::new()
                            }.into()
                        }.into(),
                    ],
                    ret: (Memory(Id::from("m5")).into(), TupleType(vec![TupleType(Vec::new()).into()]).into()),
                    env: Vec::new(),
                    allocations: vec![
                        Declaration {
                            type_: TupleType(vec![TupleType(Vec::new()).into()]).into(),
                            memory: Memory(Id::from("m5"))
                        }
                    ]
                },
            ]
        };
        "double tuple program"
    )]
    #[test_case(
        {
            let c = IntermediateMemory::from(IntermediateType::from(
                IntermediateUnionType(vec![None,None])
            ));
            let r = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateProgram{
                main: ILambda{
                    args: Vec::new(),
                    block: IBlock{
                        statements: vec![
                            IntermediateAssignment{
                                location: c.location.clone(),
                                expression:
                                    IntermediateCtorCall {
                                        idx: 0,
                                        data: None,
                                        type_: IntermediateUnionType(vec![None,None])
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                location: r.location.clone(),
                                expression: IMatch {
                                    subject: c.clone().into(),
                                    branches: vec![
                                        IntermediateMatchBranch{
                                            target: None,
                                            block: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        },
                                        IntermediateMatchBranch{
                                            target: None,
                                            block: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                        }
                                    ]
                                }.into()
                            }.into()
                        ],
                        ret: r.clone().into(),
                    },
                },
                types: vec![
                    Rc::new(RefCell::new(
                        IntermediateUnionType(vec![None,None]).into()
                    ))
                ]
            }
        },
        Program{
            type_defs: vec![
                TypeDef {
                    name: Name::from("T0"),
                    constructors: vec![
                        (Name::from("T0C0"), None),
                        (Name::from("T0C1"), None)
                    ]
                }
            ],
            fn_defs: vec![
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(), statements: vec![
                        Declaration {
                            type_: UnionType(vec![Name::from("T0C0"), Name::from("T0C1")]).into(),
                            memory: Memory(Id::from("m0"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: ConstructorCall { idx: 0, data: None }.into(),
                        }.into(),
                        Await(vec![Memory(Id::from("m0"))]).into(),
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        MatchStatement {
                            expression: (Memory(Id::from("m0")).into(), UnionType(vec![Name::from("T0C0"), Name::from("T0C1")])),
                            auxiliary_memory: Memory(Id::from("m2")),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("m1")),
                                            value: Value::from(BuiltIn::from(Integer { value: 0 })).into(),
                                        }.into()
                                    ]
                                },
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("m1")),
                                            value: Value::from(BuiltIn::from(Integer { value: 1 })).into(),
                                        }.into()
                                    ]
                                }
                            ]
                        }.into(),
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m3"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: Value::from(Memory(Id::from("m1"))).into(),
                        }.into()
                    ],
                    ret: (Memory(Id::from("m3")).into(),AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    allocations: Vec::new()
                },
            ]
        };
        "program with type defs"
    )]
    #[test_case(
        {
            let arg0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let arg1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            IntermediateProgram{
                main: ILambda{
                    args: vec![arg0, arg1.clone()],
                    block: IBlock {
                        statements: Vec::new(),
                        ret: arg1.clone().into(),
                    },
                },
                types: Vec::new()
            }
        },
        Program {
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("Main"),
                    arguments: vec![
                        (Memory(Id::from("m0")), AtomicTypeEnum::INT.into()),
                        (Memory(Id::from("m1")), AtomicTypeEnum::INT.into()),
                    ],
                    statements: Vec::new(),
                    ret: (Memory(Id::from("m1")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    allocations: Vec::new()
                }
            ]
        };
        "program with args"
    )]
    #[test_case(
        {
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let fn_ = IntermediateMemory::from(IntermediateType::from(IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            )));
            let call: IntermediateExpression = IntermediateFnCall{
                fn_: fn_.clone().into(),
                args: vec![
                    x.clone().into(),
                ]
            }.into();
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateProgram{
                types: Vec::new(),
                main: ILambda{
                    args: vec![arg.clone()],
                    block: IBlock{
                        statements: vec![
                            IntermediateAssignment{
                                expression: ILambda {
                                    args: vec![x.into()],
                                    block: IBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                location: y.location.clone(),
                                                expression: call,
                                            }.into()
                                        ],
                                        ret: y.into()
                                    },
                                }.into(),
                                location: fn_.location.clone(),
                            }.into(),
                            IntermediateAssignment{
                                location: main_call.location.clone(),
                                expression: IntermediateFnCall{
                                    fn_: fn_.clone().into(),
                                    args: vec![
                                        arg.clone().into(),
                                    ]
                                }.into(),
                            }.into()
                        ],
                        ret: main_call.clone().into(),
                    },
                }
            }
        },
        Program{
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: vec![
                        (Memory(Id::from("m1")), AtomicTypeEnum::INT.into()),
                    ],
                    statements: vec![
                        Declaration {
                            memory: Memory(Id::from("m2")),
                            type_: FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into())
                            ).into()
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: ElementAccess{
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into()
                        }.into(),
                        Await(vec![Memory(Id::from("m2")).into()]).into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: FnCall{
                                fn_: Memory(Id::from("m2")).into(),
                                args: vec![Memory(Id::from("m1")).into()],
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            }.into()
                        }.into(),
                    ],
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        MachineType::WeakFnType(FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ))
                    ].into(),
                    allocations: vec![
                        Declaration {
                            memory: Memory(Id::from("m3")),
                            type_: AtomicTypeEnum::INT.into()
                        }
                    ]
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: vec![
                        (Memory(Id::from("m0")), AtomicTypeEnum::INT.into()),
                    ],
                    statements: vec![
                        Declaration {
                            memory: Memory(Id::from("m4")),
                            type_: TupleType(vec![
                                FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            ]).into()
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: TupleExpression(vec![
                                Memory(Id::from("m5")).into(),
                            ]).into()
                        }.into(),
                        Declaration {
                            memory: Memory(Id::from("m5")),
                            type_: FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into())
                            ).into()
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m5")),
                            value: ClosureInstantiation{
                                name: Name::from("F0"),
                                env: Some(Memory(Id::from("m4")).into())
                            }.into(),
                        }.into(),
                        Await(vec![Memory(Id::from("m5")).into()]).into(),
                        Assignment {
                            target: Memory(Id::from("m6")),
                            value: FnCall{
                                fn_: Memory(Id::from("m5")).into(),
                                args: vec![Memory(Id::from("m0")).into()],
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            }.into()
                        }.into(),
                    ],
                    ret: (Memory(Id::from("m6")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    allocations: vec![
                        Declaration {
                            memory: Memory(Id::from("m6")),
                            type_: AtomicTypeEnum::INT.into()
                        }
                    ]
                }
            ],
        };
        "recursive closure program"
    )]
    fn test_compile_program(program: IntermediateProgram, expected_program: Program) {
        let mut compiler = Compiler::new();
        let compiled_program = compiler.compile_program(program);
        assert_eq!(compiled_program, expected_program);
    }

    #[fixture]
    fn temporary_filename() -> PathBuf {
        let tmp_dir = TempDir::new().expect("Could not create temp dir.");
        let tmp = tmp_dir.path().join("filename");
        tmp
    }

    #[rstest]
    fn test_compile_program_with_args(temporary_filename: PathBuf) {
        let identity = IntermediateMemory::from(IntermediateType::from(IntermediateFnType(
            vec![AtomicTypeEnum::INT.into()],
            Box::new(AtomicTypeEnum::INT.into()),
        )));
        let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
        let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
        let identity_fn = ILambda {
            args: vec![arg.clone()],
            block: IBlock {
                statements: Vec::new(),
                ret: arg.clone().into(),
            },
        };
        let program = IntermediateProgram {
            main: ILambda {
                args: Vec::new(),
                block: IBlock {
                    statements: vec![
                        IntermediateAssignment {
                            location: identity.location.clone(),
                            expression: identity_fn.clone().into(),
                        }
                        .into(),
                        IntermediateAssignment {
                            location: main_call.location.clone(),
                            expression: IntermediateFnCall {
                                fn_: identity.clone().into(),
                                args: Vec::new(),
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: main_call.clone().into(),
                },
            },
            types: Vec::new(),
        };
        let identity_vector = CodeVectorCalculator::lambda_vector(&program.main).expect("");
        Compiler::compile(
            program,
            CompilationArgs {
                export_vector_file: Some(temporary_filename.to_str().unwrap().into()),
            },
        );
        let contents = fs::read_to_string(temporary_filename).expect("Failed to read file.");
        assert_eq!(contents, identity_vector.to_string())
    }
}
