use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use crate::{
    await_deduplicator::AwaitDeduplicator, code_vector::CodeVectorCalculator, enqueuer::Enqueuer,
    statement_reorderer::StatementReorderer, weakener::Weakener, Assignment, Await, BuiltIn,
    ClosureInstantiation, CodeSizeEstimator, ConstructorCall, Declaration, ElementAccess,
    Expression, FnCall, FnDef, FnType, Id, IfStatement, MachineType, MatchBranch, MatchStatement,
    Memory, Name, Program, Statement, TranslationArgs, TupleExpression, TupleType, TypeDef,
    UnionType, Value,
};
use itertools::Itertools;
use lowering::*;
use once_cell::sync::Lazy;

const OPERATOR_NAMES: Lazy<HashMap<Id, Id>> = Lazy::new(|| {
    // Names for all the built-in operators.
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
type MemoryIds = HashMap<Register, Memory>;
type TypeLookup = HashMap<IntermediateUnionType, (Name, UnionType)>;
type FnDefs = Vec<FnDef>;

pub struct Translator {
    reference_names: ReferenceNames,
    memory_ids: MemoryIds,
    type_lookup: TypeLookup,
    fn_defs: FnDefs,
    recursive_fns: RecursiveFns,
}

impl Translator {
    pub fn new() -> Self {
        Translator {
            reference_names: ReferenceNames::new(),
            memory_ids: MemoryIds::new(),
            type_lookup: TypeLookup::new(),
            fn_defs: FnDefs::new(),
            recursive_fns: RecursiveFns::new(),
        }
    }

    fn translate_type(&self, type_: &IntermediateType) -> MachineType {
        match type_ {
            IntermediateType::AtomicType(atomic_type) => {
                let atomic_type_enum = atomic_type.0;
                atomic_type_enum.clone().into()
            }
            IntermediateType::IntermediateTupleType(IntermediateTupleType(types)) => {
                TupleType(self.translate_types(types)).into()
            }
            IntermediateType::IntermediateFnType(IntermediateFnType(arg_types, ret_type)) => {
                FnType(
                    self.translate_types(arg_types),
                    Box::new(self.translate_type(&*ret_type)),
                )
                .into()
            }
            IntermediateType::IntermediateUnionType(union_type) => {
                self.type_lookup[union_type].1.clone().into()
            }
            IntermediateType::Reference(reference) => {
                match self.reference_names.get(&reference.as_ptr()) {
                    Some(type_) => type_.clone(),
                    None => self.translate_type(&reference.borrow().clone()),
                }
            }
        }
    }
    fn translate_types(&self, types: &Vec<IntermediateType>) -> Vec<MachineType> {
        types
            .iter()
            .map(|type_| self.translate_type(type_))
            .collect()
    }
    // Translate union type definitions into structs in C++.
    fn translate_type_defs(&mut self, types: Vec<Rc<RefCell<IntermediateType>>>) -> Vec<TypeDef> {
        // Find union types.
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
        // Assign names in case of references.
        for (i, (ptr, _)) in types.iter().enumerate() {
            self.reference_names
                .insert(*ptr, MachineType::NamedType(format!("T{i}")));
        }
        // Generate constructors.
        let machine_types = types
            .iter()
            .enumerate()
            .map(|(i, (_, IntermediateUnionType(types)))| {
                let names = types
                    .iter()
                    .enumerate()
                    .map(|(j, _)| format!("T{i}C{j}"))
                    .collect_vec();
                let name = format!("T{i}");
                let intermediate_type = UnionType(names);
                self.type_lookup.insert(
                    IntermediateUnionType(types.clone()),
                    (name, intermediate_type.clone()),
                );
                (format!("T{i}"), intermediate_type)
            })
            .collect_vec();
        // Turn into `TypeDef`s.
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
                            (name, type_.as_ref().map(|type_| self.translate_type(type_)))
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

    /// Get the next unique memory address.
    fn next_memory_address(&self) -> Memory {
        Memory(format!("m{}", self.memory_ids.len()))
    }
    fn translate_register(&mut self, register: &Register) -> Memory {
        if !self.memory_ids.contains_key(register) {
            self.memory_ids
                .insert(register.clone(), self.next_memory_address());
        }
        self.memory_ids[register].clone().into()
    }
    fn translate_memory(&mut self, memory: &IntermediateMemory) -> Memory {
        self.translate_register(&memory.register)
    }
    fn translate_arg(&mut self, arg: &IntermediateArg) -> Memory {
        self.translate_register(&arg.register)
    }
    fn new_memory_register(&mut self) -> Memory {
        let register = Register::new();
        let memory = self.next_memory_address();
        self.memory_ids.insert(register.clone(), memory.clone());
        memory
    }
    fn next_fn_name(&self) -> Name {
        format!("F{}", self.fn_defs.len())
    }
    fn translate_value(&mut self, value: IntermediateValue) -> Value {
        match &value {
            IntermediateValue::IntermediateArg(arg) => self.translate_arg(arg).into(),
            IntermediateValue::IntermediateMemory(memory) => self.translate_memory(memory).into(),
            IntermediateValue::IntermediateBuiltIn(built_in) => Value::from(match built_in {
                IntermediateBuiltIn::Boolean(boolean) => BuiltIn::from(boolean.clone()),
                IntermediateBuiltIn::Integer(integer) => BuiltIn::from(integer.clone()),
                IntermediateBuiltIn::BuiltInFn(BuiltInFn(name, _)) => {
                    BuiltIn::BuiltInFn(OPERATOR_NAMES[name].clone()).into()
                }
            }),
        }
    }
    fn translate_values(&mut self, values: Vec<IntermediateValue>) -> Vec<Value> {
        values
            .into_iter()
            .map(|value| self.translate_value(value))
            .collect()
    }
    /// Expressions may need multiple lines to be defined (e.g. if statement) so return any new statements too.
    fn translate_expression(
        &mut self,
        expression: IntermediateExpression,
    ) -> (Vec<Statement>, Expression) {
        match expression {
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                let values = self.translate_values(values);
                (Vec::new(), TupleExpression(values).into())
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                let value = self.translate_value(value);
                (Vec::new(), ElementAccess { value, idx }.into())
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                let MachineType::FnType(fn_type) = self.translate_type(&fn_.type_()) else {
                    panic!("Function has non-function type.")
                };
                let fn_value = self.translate_value(fn_);
                let args_values = self.translate_values(args);
                (
                    if let Value::Memory(mem) = &fn_value {
                        // Ensure fn is calculated before call.
                        vec![Await(vec![mem.clone()]).into()]
                    } else {
                        let memory = args_values
                            .iter()
                            .filter_map(Value::filter_memory)
                            .collect_vec();
                        if memory.len() > 0 {
                            vec![Await(memory).into()]
                        } else {
                            Vec::new()
                        }
                    },
                    FnCall {
                        fn_: fn_value,
                        fn_type,
                        args: args_values,
                    }
                    .into(),
                )
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => {
                let value = data.map(|data| self.translate_value(data));
                let (name, _) = &self.type_lookup[&type_];
                (
                    Vec::new(),
                    ConstructorCall {
                        type_: name.clone(),
                        idx,
                        data: value.map(|value| {
                            let MachineType::UnionType(UnionType(variants)) =
                                self.translate_type(&type_.into())
                            else {
                                panic!("Did not translate union type into union type.")
                            };
                            (variants[idx].clone(), value)
                        }),
                    }
                    .into(),
                )
            }
            IntermediateExpression::IntermediateValue(value) => {
                let value = self.translate_value(value);
                (Vec::new(), value.into())
            }
            IntermediateExpression::IntermediateLambda(lambda) => {
                let (statements, closure_inst) = self.translate_lambda(lambda);
                (statements, closure_inst.into())
            }
            IntermediateExpression::IntermediateIf(if_) => {
                let (statements, value) = self.translate_if(if_);
                (statements, value.into())
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                let (statements, value) = self.translate_match(match_);
                (statements, value.into())
            }
        }
    }
    fn translate_if(&mut self, if_: IntermediateIf) -> (Vec<Statement>, Value) {
        let IntermediateIf {
            condition,
            branches: (true_block, false_block),
        } = if_;
        let target = IntermediateMemory::from(true_block.type_());
        let memory = self.translate_memory(&target);
        // Add declaration for shared result.
        let mut statements = vec![Declaration {
            memory: memory.clone(),
            type_: self.translate_type(&target.type_()),
        }
        .into()];
        let condition = self.translate_value(condition);
        if let Value::Memory(mem) = &condition {
            statements.push(Await(vec![mem.clone()]).into())
        };
        let (mut true_statements, true_value) = self.translate_block(true_block);
        true_statements.push(
            Assignment {
                target: memory.clone(),
                value: true_value.into(),
            }
            .into(),
        );
        let (mut false_statements, false_value) = self.translate_block(false_block);
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
        (statements, memory.into())
    }
    fn translate_match(&mut self, match_: IntermediateMatch) -> (Vec<Statement>, Value) {
        let IntermediateMatch { subject, branches } = match_;
        let type_ = subject.type_();
        let MachineType::UnionType(union_type) = self.translate_type(&type_) else {
            panic!("Match expression subject has non-union type.")
        };
        let result = IntermediateMemory::from(branches[0].block.type_());
        let memory = self.translate_memory(&result);
        // Add declaration for shared result.
        let mut statements = vec![Declaration {
            memory: memory.clone(),
            type_: self.translate_type(&result.type_()),
        }
        .into()];
        let subject = self.translate_value(subject);
        if let Value::Memory(mem) = &subject {
            statements.push(Await(vec![mem.clone()]).into());
        };
        let branches = branches
            .into_iter()
            .map(|IntermediateMatchBranch { target, block }| {
                let (mut statements, value) = self.translate_block(block);
                statements.push(
                    Assignment {
                        target: memory.clone(),
                        value: value.into(),
                    }
                    .into(),
                );
                MatchBranch {
                    target: target.map(|arg| self.translate_arg(&arg)),
                    statements,
                }
            })
            .collect();
        statements.push(
            MatchStatement {
                expression: (subject, union_type),
                branches,
                auxiliary_memory: self.new_memory_register(),
            }
            .into(),
        );
        let value = self.translate_memory(&result);
        (statements, value.into())
    }
    fn translate_statement(&mut self, statement: IntermediateStatement) -> Vec<Statement> {
        match statement {
            IntermediateStatement::IntermediateAssignment(memory) => {
                self.translate_assignment(memory)
            }
        }
    }
    fn translate_assignment(&mut self, assignment: IntermediateAssignment) -> Vec<Statement> {
        let IntermediateAssignment {
            expression,
            register,
        } = assignment;
        let type_ = self.translate_type(&expression.type_());
        let (mut statements, value) = self.translate_expression(expression);
        let memory = self.translate_register(&register);
        let assignment = Assignment {
            target: memory.clone(),
            value: value.clone(),
        };
        match &value {
            Expression::ClosureInstantiation(_) => {
                // Closures require declarations so that fns can be mutually recursive.
                statements.push(
                    Declaration {
                        memory: memory.clone().into(),
                        type_,
                    }
                    .into(),
                );
            }
            _ => {}
        }
        statements.push(assignment.into());
        statements
    }
    fn translate_statements(&mut self, statements: Vec<IntermediateStatement>) -> Vec<Statement> {
        statements
            .into_iter()
            .flat_map(|statement| self.translate_statement(statement))
            .collect()
    }
    fn translate_block(&mut self, block: IntermediateBlock) -> (Vec<Statement>, Value) {
        let statements = block
            .statements
            .into_iter()
            .flat_map(|statement| self.translate_statement(statement))
            .collect();
        let value = self.translate_value(block.ret);
        (statements, value)
    }
    /// Substitute open variables in lambdas with new variables.
    fn replace_open_vars(
        &mut self,
        fn_def: &mut IntermediateLambda,
    ) -> Vec<(IntermediateValue, Register)> {
        let open_vars = fn_def.find_open_vars();
        let new_registers = open_vars
            .iter()
            .map(|val| IntermediateMemory::from(val.type_().clone()))
            .collect_vec();
        let substitution = open_vars
            .iter()
            .zip(new_registers.iter())
            .map(|(var, mem)| (var.register().unwrap().clone(), mem.register.clone()))
            .collect::<HashMap<_, _>>();
        fn_def.substitute(&substitution);
        open_vars
            .iter()
            .zip(new_registers.iter())
            .map(|(val, mem)| (val.clone().into(), mem.register.clone()))
            .collect()
    }
    fn closure_prefix(&mut self, env_registers: &Vec<Register>) -> Vec<Statement> {
        // Prefix closures by spilling environment tuple.
        env_registers
            .iter()
            .enumerate()
            .flat_map(|(i, register)| {
                let memory = self.translate_register(register);
                vec![Assignment {
                    target: memory,
                    value: ElementAccess {
                        idx: i,
                        value: Memory(Id::from("env")).into(),
                    }
                    .into(),
                }
                .into()]
            })
            .collect_vec()
    }
    fn translate_lambda(
        &mut self,
        mut lambda: IntermediateLambda,
    ) -> (Vec<Statement>, ClosureInstantiation) {
        let is_recursive = self.recursive_fns.get(&lambda).cloned().unwrap_or(false);
        // Replace open variables to determine environment.
        let env_values = self.replace_open_vars(&mut lambda);
        // Determine types of open variables.
        let env_types = env_values
            .iter()
            .map(|(value, _)| self.translate_type(&value.type_()))
            .collect_vec();
        let env_registers = env_values
            .iter()
            .map(|(_, register)| register.clone())
            .collect_vec();

        let size = CodeSizeEstimator::estimate_size(&lambda);
        let IntermediateLambda {
            args,
            block:
                IntermediateBlock {
                    statements,
                    ret: return_value,
                },
        } = lambda;
        let args = args
            .into_iter()
            .map(|arg| (self.translate_arg(&arg), self.translate_type(&arg.type_())))
            .collect_vec();
        let mut prefix = self.closure_prefix(&env_registers);
        let mut statements = self.translate_statements(statements);
        prefix.extend(statements);
        statements = prefix;
        let ret_type = self.translate_type(&return_value.type_());
        let ret_val = self.translate_value(return_value);
        let name = self.next_fn_name();
        self.fn_defs.push(FnDef {
            name: name.clone(),
            arguments: args,
            statements,
            ret: (ret_val, ret_type),
            env: env_types.clone(),
            size_bounds: size,
            is_recursive,
        });

        if env_values.len() > 0 {
            // Define closure as a tuple.
            let tuple_mem = self.new_memory_register();
            let values = env_values
                .into_iter()
                .map(|(value, _)| self.translate_value(value))
                .collect();
            // Assign to all closed variables.
            let statements = vec![Assignment {
                target: tuple_mem.clone(),
                value: TupleExpression(values).into(),
            }
            .into()];
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
    fn translate_program(&mut self, program: IntermediateProgram) -> Program {
        self.recursive_fns = RecursiveFnFinder::recursive_fns(&program);
        let IntermediateProgram { main, types } = program;
        let type_defs = self.translate_type_defs(types);
        let (statements, _) = self.translate_lambda(main);
        // Check that main has no open variables.
        assert_eq!(statements.len(), 0);
        // Main is the last translated program.
        let main = self.fn_defs.last_mut().unwrap();
        main.name = Name::from("Main");
        let program = Program {
            fn_defs: self.fn_defs.clone(),
            type_defs,
        };
        let program = Weakener::weaken(program);
        let program = StatementReorderer::reorder(program);
        let program = AwaitDeduplicator::deduplicate(program);
        let program = Enqueuer::enqueue(program);
        program
    }
    pub fn translate(program: IntermediateProgram, args: TranslationArgs) -> Program {
        let mut translator = Translator::new();
        if let Some(filename) = args.export_vector_file {
            Self::export_vector(&program, filename).expect("Failed to save program")
        };
        translator.translate_program(program)
    }
    /// Export code vectors to a file.
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

    use crate::{CodeSizeEstimator, Enqueue};

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
    fn test_translate_type_defs(
        type_defs: Vec<Rc<RefCell<IntermediateType>>>,
        expected_type_defs: Vec<TypeDef>,
    ) {
        let mut translator = Translator::new();
        assert_eq!(
            translator.translate_type_defs(type_defs),
            expected_type_defs
        )
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
    fn test_translate_values(value: IntermediateValue, expected_value: Value) {
        let mut translator = Translator::new();
        let translated_value = translator.translate_value(value);
        assert_eq!(translated_value, expected_value);
    }
    #[test]
    fn test_translate_multiple_memory_registers() {
        let registers = vec![Register::new(), Register::new(), Register::new()];
        let mut translator = Translator::new();
        let value_0 = translator.translate_register(&registers[0].clone());
        let value_1 = translator.translate_register(&registers[1].clone());
        let value_2 = translator.translate_register(&registers[2].clone());
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(
            value_0,
            translator.translate_register(&registers[0].clone())
        );
        assert_eq!(
            value_1,
            translator.translate_register(&registers[1].clone())
        );
        assert_eq!(
            value_2,
            translator.translate_register(&registers[2].clone())
        );
    }
    #[test]
    fn test_translate_arguments() {
        let types: Vec<IntermediateType> = vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
            AtomicTypeEnum::INT.into(),
        ];
        let mut translator = Translator::new();

        let args = types
            .into_iter()
            .map(|type_| IntermediateArg::from(type_))
            .collect_vec();
        let value_0 = translator.translate_value(args[0].clone().into());
        let value_1 = translator.translate_value(args[1].clone().into());
        let value_2 = translator.translate_value(args[2].clone().into());
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(value_0, translator.translate_value(args[0].clone().into()));
        assert_eq!(value_1, translator.translate_value(args[1].clone().into()));
        assert_eq!(value_2, translator.translate_value(args[2].clone().into()));
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
            fn_: BuiltInFn(
                Name::from("**"),
                IntermediateFnType(
                    vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::INT.into(),
                    ],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            ).into(),
            args: vec![
                IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT)).into(),
                IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT)).into(),
            ]
        }.into(),
        (
            vec![
                Await(vec![Memory(Id::from("m0")), Memory(Id::from("m1"))]).into()
            ],
            FnCall{
                args: vec![
                    Memory(Id::from("m0")).into(),
                    Memory(Id::from("m1")).into(),
                ],
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Exponentiate__BuiltIn"),
                ).into(),
                fn_type: FnType(
                    vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::INT.into(),
                    ],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            }.into()
        );
        "built-in fn call await"
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
    fn test_translate_expressions(
        expression: IntermediateExpression,
        expected: (Vec<Statement>, Expression),
    ) {
        let mut translator = Translator::new();
        let (statements, expression) = translator.translate_expression(expression);
        assert_eq!((statements, expression), expected);
    }

    #[test_case(
        {
            let type_ = IntermediateUnionType(vec![None, None]);
            (
                IntermediateCtorCall{
                    idx: 0,
                    data: None,
                    type_: type_.clone()
                }.into(),
                Rc::new(RefCell::new(type_.into()))
            )
        },
        (
            Vec::new(),
            ConstructorCall{
                type_: Name::from("T0"),
                idx: 0,
                data: None
            }.into()
        );
        "no data constructor call"
    )]
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
                type_: Name::from("T0"),
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
                type_: Name::from("T0"),
                idx: 0,
                data: Some((Name::from("T0C0"), Memory(Id::from("m0")).into()))
            }.into()
        );
        "recursive constructor call"
    )]
    fn test_translate_constructors(
        constructor_type: (IntermediateCtorCall, Rc<RefCell<IntermediateType>>),
        expected: (Vec<Statement>, Expression),
    ) {
        let (constructor, type_) = constructor_type;
        let mut translator = Translator::new();
        translator.translate_type_defs(vec![type_]);
        let (statements, expression) = translator.translate_expression(constructor.into());
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
                register: Register::new()
            }.into()
        ],
        vec![
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
                register: Register::new()
            }.into()
        ],
        vec![
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
                register: Register::new()
            }.into()
        ],
        vec![
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
            let register = Register::new();
            vec![
                IntermediateAssignment {
                    register: register.clone(),
                    expression: IntermediateIf{
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
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            IfStatement {
                condition: Memory(Id::from("m1")).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 1}).into()
                            ),
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 0}).into()
                            ),
                        }.into(),
                    ],
                )
            }.into(),
            Assignment {
                target: Memory(Id::from("m2")),
                value: Expression::Value(Memory(Id::from("m0")).into()),
            }.into(),
        ];
        "if statement awaited argument"
    )]
    #[test_case(
        {
            let register = Register::new();
            vec![
                IntermediateAssignment {
                    register: register.clone(),
                    expression: IntermediateIf{
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
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Value(Memory(Id::from("m0")).into()),
            }.into(),
        ];
        "if statement value only"
    )]
    #[test_case(
        {
            let register = Register::new();
            let temp = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            vec![
                IntermediateAssignment {
                    register: register.clone(),
                    expression: IntermediateIf{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            (
                                vec![
                                    IntermediateAssignment {
                                        register: temp.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: IntermediateMemory{
                                                register: Register::new(),
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
            let register = Register::new();
            vec![
                IntermediateAssignment{
                    register: register,
                    expression: IntermediateLambda {
                        args: vec![arg.clone()],
                        block: IntermediateBlock {
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
    fn test_translate_statements(
        statements: Vec<IntermediateStatement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut translator = Translator::new();
        let translated_statements = translator.translate_statements(statements);
        assert_eq!(translated_statements, expected_statements);
    }
    #[test_case(
        {
            let bull_type: IntermediateType = IntermediateUnionType(vec![None,None]).into();
            let arg: IntermediateArg = IntermediateType::from(bull_type.clone()).into();
            let register = Register::new();
            (
                vec![Rc::new(RefCell::new(bull_type))],
                vec![
                    IntermediateAssignment {
                        register: register.clone(),
                        expression: IntermediateMatch{
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
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            MatchStatement {
                auxiliary_memory: Memory(Id::from("m2")),
                expression: (
                    Memory(Id::from("m1")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m0")),
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
                                target: Memory(Id::from("m0")),
                                value: Expression::Value(
                                    BuiltIn::from(Integer{value: 0}).into()
                                ),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: Expression::Value(Memory(Id::from("m0")).into()),
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
                        register: memory.register.clone(),
                        expression: IntermediateMatch {
                            subject: arg.into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(target0.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment {
                                                register: temp.register.clone(),
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
                        register: Register::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![memory.clone().into(), IntermediateBuiltIn::from(Integer{value: 0}).into()]
                            ).into()
                    }.into(),
                    IntermediateAssignment {
                        register: Register::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![memory.clone().into(), IntermediateBuiltIn::from(Integer{value: 1}).into()]
                            ).into()
                    }.into()
                ]
            )
        },
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m1")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                auxiliary_memory: Memory(Id::from("m5")),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("m2"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("m2"))]).into(),
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
                                target: Memory(Id::from("m0")),
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
                                target: Memory(Id::from("m0")),
                                value: Expression::Value(
                                    Memory(Id::from("m4")).into(),
                                ),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Assignment {
                target: Memory(Id::from("m6")),
                value: Value::from(Memory(Id::from("m0"))).into()
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
    fn test_translate_match_statements(
        args_types_statements: (
            Vec<Rc<RefCell<IntermediateType>>>,
            Vec<IntermediateStatement>,
        ),
        expected_statements: Vec<Statement>,
    ) {
        let (types, statements) = args_types_statements;
        let mut translator = Translator::new();
        translator.translate_type_defs(types);
        let translated_statements = translator.translate_statements(statements);
        assert_eq!(translated_statements, expected_statements);
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
            IntermediateLambda {
                args: vec![arg0.clone(), arg1.clone()],
                block: IntermediateBlock{
                    statements: vec![
                        IntermediateAssignment{
                            register: y.register.clone(),
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
                    Await(vec![Memory(Id::from("m0")),Memory(Id::from("m1"))]).into(),
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
                size_bounds: (0, 0),
                is_recursive: false
            },
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
            IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: vec![
                        IntermediateAssignment{
                            register: z.register.clone(),
                            expression: z_expression,
                        }.into()
                    ],
                    ret: z.into()
                },
            }
        },
        (
            vec![
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
                    Assignment {
                        target: Memory(Id::from("m0")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 0
                        }.into()
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m1")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 1
                        }.into()
                    }.into(),
                    Await(vec![Memory(Id::from("m0")),Memory(Id::from("m1"))]).into(),
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
                size_bounds: (0, 0),
                is_recursive: false
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
            IntermediateLambda {
                args: vec![y.clone()],
                block: IntermediateBlock{
                    statements: vec![
                        IntermediateAssignment{
                            register: z.register.clone(),
                            expression: z_expression,
                        }.into()
                    ],
                    ret: z.into()
                },
            }
        },
        (
            vec![
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
                    Assignment {
                        target: Memory(Id::from("m1")),
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 0
                        }.into()
                    }.into(),
                    Await(vec![Memory(Id::from("m1")),Memory(Id::from("m0"))]).into(),
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
                size_bounds: (0, 0),
                is_recursive: false
            }
        );
        "env and argument"
    )]
    fn test_translate_fn_defs(
        fn_def: IntermediateLambda,
        expected: (Vec<Statement>, ClosureInstantiation, FnDef),
    ) {
        let (expected_statements, expected_value, expected_fn_def) = expected;

        let mut translator = Translator::new();
        let size = CodeSizeEstimator::estimate_size(&fn_def);
        let translated = translator.translate_lambda(fn_def);
        assert_eq!(translated, (expected_statements, expected_value));
        let translated_fn_def = &translator.fn_defs[0];
        assert_eq!(translated_fn_def, &expected_fn_def);
        assert_eq!(translated_fn_def.size_bounds, size);
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
                main: IntermediateLambda{
                    args: Vec::new(),
                    block: IntermediateBlock{
                        ret: main_call.clone().into(),
                        statements: vec![
                            IntermediateAssignment{
                                register: identity.register.clone(),
                                expression: IntermediateLambda{
                                    args: vec![arg.clone()],
                                    block: IntermediateBlock {
                                        statements: Vec::new(),
                                        ret: arg.clone().into()
                                    },
                                }.into()
                            }.into(),
                            IntermediateAssignment{
                                register: main.register.clone(),
                                expression:
                                    IntermediateLambda{
                                        args: Vec::new(),
                                        block: IntermediateBlock{
                                            statements: vec![
                                                IntermediateAssignment{
                                                    register: y.register.clone(),
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
                                register: main_call.register.clone(),
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
                    statements: vec![
                        Enqueue(Memory(Id::from("m0"))).into(),
                    ],
                    ret: (Memory(Id::from("m0")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: Vec::new(),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: ElementAccess {
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m2"))).into(),
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
                        }.into(),
                        Enqueue(Memory(Id::from("m3"))).into(),
                    ],
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ].into(),
                    size_bounds: (0, 0),
                    is_recursive: false
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
                        Enqueue(Memory(Id::from("m5"))).into(),
                        Await(vec![Memory(Id::from("m5"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m6")),
                            value: FnCall {
                                fn_: Memory(Id::from("m5")).into(),
                                fn_type: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                                args: Vec::new()
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m6"))).into(),
                    ],
                    ret: (Memory(Id::from("m6")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
                }
            ]
        };
        "identity call program"
    )]
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
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT),
            );
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            IntermediateProgram {
                main: IntermediateLambda{
                    args: Vec::new(),
                    block: IntermediateBlock{
                        ret: main_call.clone().into(),
                        statements: vec![
                            IntermediateAssignment{
                                register: identity.register.clone(),
                                expression: IntermediateLambda{
                                    args: vec![arg.clone()],
                                    block: IntermediateBlock {
                                        statements: Vec::new(),
                                        ret: arg.clone().into()
                                    },
                                }.into()
                            }.into(),
                            IntermediateAssignment{
                                register: main.register.clone(),
                                expression:
                                    IntermediateLambda{
                                        args: vec![IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))],
                                        block: IntermediateBlock{
                                            statements: vec![
                                                IntermediateAssignment{
                                                    register: y.register.clone(),
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
                                register: z.register.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: vec![Integer{value: 0}.into()]
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                register: main_call.register.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: vec![z.clone().into()]
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
                    statements: vec![
                        Enqueue(Memory(Id::from("m0"))).into(),
                    ],
                    ret: (Memory(Id::from("m0")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: vec![(Memory(Id::from("m2")), AtomicTypeEnum::INT.into())],
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: ElementAccess {
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m3"))).into(),
                        Await(vec![Memory(Id::from("m3"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: FnCall {
                                fn_: Memory(Id::from("m3")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ),
                                args: vec![BuiltIn::from(Integer { value: 0 }).into()]
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m4"))).into(),
                    ],
                    ret: (Memory(Id::from("m4")).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ].into(),
                    size_bounds: (0, 0),
                    is_recursive: false
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
                        Assignment {
                            target: Memory(Id::from("m5")),
                            value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
                        }.into(),
                        Declaration {
                            type_: FnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                            memory: Memory(Id::from("m6"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m6")),
                            value: ClosureInstantiation {
                                name: Name::from("F1"),
                                env: Some(Memory(Id::from("m5")).into())
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m6"))).into(),
                        Await(vec![Memory(Id::from("m6"))]).into(),
                        Assignment {
                            target: Memory(Id::from("m7")),
                            value: FnCall {
                                fn_: Memory(Id::from("m6")).into(),
                                fn_type: FnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                                args: vec![Integer{value: 0}.into()]
                            }.into(),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m8")),
                            value: FnCall {
                                fn_: Memory(Id::from("m6")).into(),
                                fn_type: FnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                                args: vec![Memory(Id::from("m7")).into()]
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m8"))).into(),
                    ],
                    ret: (Memory(Id::from("m8")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
                }
            ]
        };
        "double await program"
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
                main: IntermediateLambda{
                    args: Vec::new(),
                    block: IntermediateBlock{
                        ret: main_call.clone().into(),
                        statements: vec![
                            IntermediateAssignment{
                                register: t1.register.clone(),
                                expression:
                                    IntermediateTupleExpression(Vec::new()).into()
                            }.into(),
                            IntermediateAssignment{
                                register: t2.register.clone(),
                                expression:
                                    IntermediateTupleExpression(vec![t1.clone().into()]).into()
                            }.into(),
                            IntermediateAssignment{
                                register: main.register.clone(),
                                expression:
                                    IntermediateLambda{
                                        args: Vec::new(),
                                        block: IntermediateBlock {
                                            statements: Vec::new(),
                                            ret: t2.clone().into()
                                        },
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                register: main_call.register.clone(),
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
                        Assignment{
                            target: Memory(Id::from("m2")),
                            value: ElementAccess{
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into()
                        }.into(),
                        Enqueue(Memory(Id::from("m2"))).into(),
                    ],
                    ret: (Memory(Id::from("m2")).into(), TupleType(vec![TupleType(Vec::new()).into()]).into()),
                    env: vec![TupleType(vec![TupleType(Vec::new()).into()]).into()].into(),
                    size_bounds: (0, 0),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: TupleExpression(Vec::new()).into(),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: TupleExpression(vec![Memory(Id::from("m0")).into()]).into(),
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
                        Enqueue(Memory(Id::from("m4"))).into(),
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
                        Enqueue(Memory(Id::from("m5"))).into(),
                    ],
                    ret: (Memory(Id::from("m5")).into(), TupleType(vec![TupleType(Vec::new()).into()]).into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
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
                main: IntermediateLambda{
                    args: Vec::new(),
                    block: IntermediateBlock{
                        statements: vec![
                            IntermediateAssignment{
                                register: c.register.clone(),
                                expression:
                                    IntermediateCtorCall {
                                        idx: 0,
                                        data: None,
                                        type_: IntermediateUnionType(vec![None,None])
                                    }.into()
                            }.into(),
                            IntermediateAssignment{
                                register: r.register.clone(),
                                expression: IntermediateMatch {
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
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: ConstructorCall { idx: 0, data: None, type_: Name::from("T0"), }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("m0"))).into(),
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        Await(vec![Memory(Id::from("m0"))]).into(),
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
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: Value::from(Memory(Id::from("m1"))).into(),
                        }.into(),
                    ],
                    ret: (Memory(Id::from("m3")).into(),AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
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
                main: IntermediateLambda{
                    args: vec![arg0, arg1.clone()],
                    block: IntermediateBlock {
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
                    statements: vec![
                        Enqueue(Memory(Id::from("m1"))).into(),
                    ],
                    ret: (Memory(Id::from("m1")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0,0),
                    is_recursive: false
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
                main: IntermediateLambda{
                    args: vec![arg.clone()],
                    block: IntermediateBlock{
                        statements: vec![
                            IntermediateAssignment{
                                expression: IntermediateLambda {
                                    args: vec![x.into()],
                                    block: IntermediateBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                register: y.register.clone(),
                                                expression: call,
                                            }.into()
                                        ],
                                        ret: y.into()
                                    },
                                }.into(),
                                register: fn_.register.clone(),
                            }.into(),
                            IntermediateAssignment{
                                register: main_call.register.clone(),
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
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: ElementAccess{
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into()
                        }.into(),
                        Enqueue(Memory(Id::from("m2"))).into(),
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
                        Enqueue(Memory(Id::from("m3"))).into(),
                    ],
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        MachineType::WeakFnType(FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ))
                    ].into(),
                    size_bounds: (0, 0),
                    is_recursive: true
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: vec![
                        (Memory(Id::from("m0")), AtomicTypeEnum::INT.into()),
                    ],
                    statements: vec![
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
                        Enqueue(Memory(Id::from("m5"))).into(),
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
                        Enqueue(Memory(Id::from("m6"))).into(),
                    ],
                    ret: (Memory(Id::from("m6")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    size_bounds: (0, 0),
                    is_recursive: false
                }
            ],
        };
        "recursive closure program"
    )]
    fn test_translate_program(program: IntermediateProgram, expected_program: Program) {
        let mut translator = Translator::new();
        let main_size = CodeSizeEstimator::estimate_size(&program.main);
        let translated_program = translator.translate_program(program);
        assert_eq!(expected_program, translated_program);
        let main = translated_program.fn_defs.last().unwrap();
        assert_eq!(main.size_bounds, main_size);
    }

    #[fixture]
    fn temporary_filename() -> PathBuf {
        let tmp_dir = TempDir::new().expect("Could not create temp dir.");
        let tmp = tmp_dir.path().join("filename");
        tmp
    }

    #[rstest]
    fn test_translate_program_with_args(temporary_filename: PathBuf) {
        let identity = IntermediateMemory::from(IntermediateType::from(IntermediateFnType(
            vec![AtomicTypeEnum::INT.into()],
            Box::new(AtomicTypeEnum::INT.into()),
        )));
        let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
        let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
        let identity_fn = IntermediateLambda {
            args: vec![arg.clone()],
            block: IntermediateBlock {
                statements: Vec::new(),
                ret: arg.clone().into(),
            },
        };
        let program = IntermediateProgram {
            main: IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            register: identity.register.clone(),
                            expression: identity_fn.clone().into(),
                        }
                        .into(),
                        IntermediateAssignment {
                            register: main_call.register.clone(),
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
        Translator::translate(
            program,
            TranslationArgs {
                export_vector_file: Some(temporary_filename.to_str().unwrap().into()),
            },
        );
        let contents = fs::read_to_string(temporary_filename).expect("Failed to read file.");
        assert_eq!(contents, identity_vector.to_string())
    }
}
