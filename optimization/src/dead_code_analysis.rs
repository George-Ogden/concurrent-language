use std::{
    cmp::minmax,
    collections::{HashMap, HashSet, VecDeque},
    convert::identity,
    iter,
};

use itertools::{zip_eq, Itertools};
use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateBlock, IntermediateExpression,
    IntermediateFnCall, IntermediateFnType, IntermediateIf, IntermediateLambda, IntermediateMatch,
    IntermediateMatchBranch, IntermediateMemory, IntermediateProgram, IntermediateStatement,
    IntermediateTupleExpression, IntermediateType, IntermediateValue, Register,
};

pub struct DeadCodeAnalyzer {
    single_constraints: HashMap<Register, HashSet<Register>>,
    double_constraints: HashMap<(Register, Register), HashSet<Register>>,
    fn_args: HashMap<Register, Vec<Register>>,
    variables: HashSet<Register>,
    fn_updates: HashMap<Register, Register>,
}

impl DeadCodeAnalyzer {
    pub fn new() -> Self {
        DeadCodeAnalyzer {
            single_constraints: HashMap::new(),
            double_constraints: HashMap::new(),
            fn_args: HashMap::new(),
            variables: HashSet::new(),
            fn_updates: HashMap::new(),
        }
    }
    fn used_value(&mut self, value: &IntermediateValue) -> Option<Register> {
        match value {
            lowering::IntermediateValue::IntermediateMemory(memory) => {
                Some(memory.register.clone())
            }
            lowering::IntermediateValue::IntermediateArg(arg) => Some(arg.register.clone()),
            lowering::IntermediateValue::IntermediateBuiltIn(_) => None,
        }
    }
    fn find_used_values(&mut self, expression: &IntermediateExpression) -> Vec<Register> {
        let values = expression.values();
        values
            .into_iter()
            .filter_map(|value| self.used_value(&value))
            .collect()
    }
    fn add_single_constraint(&mut self, register: Register, dependents: Vec<Register>) {
        if !self.single_constraints.contains_key(&register) {
            self.single_constraints
                .insert(register.clone(), HashSet::new());
        }
        self.single_constraints
            .get_mut(&register)
            .unwrap()
            .extend(dependents);
    }
    fn add_double_constraint(
        &mut self,
        arg: Register,
        register: Register,
        dependents: Vec<Register>,
    ) {
        // Sort registers so that the smallest is on the left.
        let key = minmax(arg, register).into();
        if !self.double_constraints.contains_key(&key) {
            self.double_constraints.insert(key.clone(), HashSet::new());
        }
        self.double_constraints
            .get_mut(&key)
            .unwrap()
            .extend(dependents);
    }
    fn generate_constraints(&mut self, statements: &Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => match &expression {
                    IntermediateExpression::IntermediateLambda(IntermediateLambda {
                        args,
                        block:
                            IntermediateBlock {
                                statements: _,
                                ret: _,
                            },
                    }) => {
                        // Register fn args.
                        let args = args.into_iter().map(|arg| arg.register.clone()).collect();
                        self.fn_args.insert(register.clone(), args);
                    }
                    _ => {}
                },
            }
        }
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => match &expression {
                    IntermediateExpression::IntermediateLambda(IntermediateLambda {
                        args: _,
                        block: IntermediateBlock { statements, ret },
                    }) => {
                        self.generate_constraints(statements);
                        let dependents = self.used_value(&ret).iter().cloned().collect_vec();
                        self.add_single_constraint(register.clone(), dependents);
                    }
                    IntermediateExpression::IntermediateIf(IntermediateIf {
                        condition,
                        branches,
                    }) => {
                        let dependents = [
                            self.used_value(&branches.0.ret),
                            self.used_value(&branches.1.ret),
                            self.used_value(&condition),
                        ]
                        .into_iter()
                        .filter_map(identity)
                        .collect();
                        self.add_single_constraint(register.clone(), dependents);
                        self.generate_constraints(&branches.0.statements);
                        self.generate_constraints(&branches.1.statements);
                    }
                    IntermediateExpression::IntermediateMatch(IntermediateMatch {
                        subject,
                        branches,
                    }) => {
                        let dependents = iter::once(self.used_value(subject))
                            .chain(branches.iter().map(|branch| {
                                self.generate_constraints(&branch.block.statements);
                                self.used_value(&branch.block.ret)
                            }))
                            .filter_map(identity)
                            .collect();
                        self.add_single_constraint(register.clone(), dependents);
                    }
                    IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                        fn_,
                        args,
                    }) => match fn_ {
                        IntermediateValue::IntermediateBuiltIn(_) => {
                            let dependents = args
                                .iter()
                                .filter_map(|value| self.used_value(value))
                                .collect();
                            self.add_single_constraint(register.clone(), dependents);
                        }
                        IntermediateValue::IntermediateMemory(fn_) => {
                            self.add_single_constraint(
                                register.clone(),
                                vec![fn_.register.clone()],
                            );
                            match self.fn_args.get(&fn_.register) {
                                Some(fn_args) => {
                                    for (reg, arg) in zip_eq(fn_args.clone(), args) {
                                        // Require that the parameter is used in the fn definition and the return value is used in order for the argument to count as used.
                                        let dependents =
                                            self.used_value(arg).iter().cloned().collect_vec();
                                        self.add_double_constraint(
                                            reg,
                                            register.clone(),
                                            dependents,
                                        )
                                    }
                                }
                                None => {
                                    let dependents = args
                                        .iter()
                                        .filter_map(|arg| self.used_value(arg))
                                        .collect();
                                    self.add_single_constraint(register.clone(), dependents);
                                }
                            }
                        }
                        _ => {
                            let mut values = args.clone();
                            values.push(fn_.clone());
                            self.generate_constraints(&vec![IntermediateAssignment {
                                expression: IntermediateTupleExpression(values).into(),
                                register: register.clone(),
                            }
                            .into()]);
                        }
                    },
                    expression => {
                        let used_values = self.find_used_values(&expression);
                        self.add_single_constraint(register.clone(), used_values)
                    }
                },
            }
        }
    }
    fn solve_constraints(&self, initial_solution: Vec<Register>) -> HashSet<Register> {
        let mut solution = HashSet::from_iter(initial_solution.clone());
        let mut new_variables = VecDeque::from(initial_solution);
        // Build index of double constraints so we can check them quickly whenever a new variable is freed.
        let mut double_constraint_index: HashMap<Register, Vec<Register>> = HashMap::from_iter(
            self.double_constraints
                .keys()
                .flat_map(|(r1, r2)| [(r1.clone(), Vec::new()), (r2.clone(), Vec::new())]),
        );
        for (k, v) in self
            .double_constraints
            .keys()
            .flat_map(|(r1, r2)| [(r1.clone(), r2.clone()), (r2.clone(), r1.clone())])
        {
            double_constraint_index.get_mut(&k).unwrap().push(v);
        }
        while let Some(c) = new_variables.pop_front() {
            if let Some(set) = self.single_constraints.get(&c) {
                for variable in set {
                    if !solution.contains(&variable) {
                        solution.insert(variable.clone());
                        new_variables.push_back(variable.clone());
                    }
                }
            }
            if let Some(others) = double_constraint_index.get(&c) {
                for other in others {
                    if solution.contains(other) {
                        let key = minmax(c.clone(), other.clone()).into();
                        for variable in &self.double_constraints[&key] {
                            if !solution.contains(&variable) {
                                solution.insert(variable.clone());
                                new_variables.push_back(variable.clone());
                            }
                        }
                    }
                }
            }
        }
        solution
    }
    /// Filter values based on whether args for the fn in register are used.
    fn filter_args<T>(&self, register: &Register, values: Vec<T>) -> Vec<T> {
        match self.fn_args.get(&register) {
            None => values,
            Some(args) => values
                .into_iter()
                .zip_eq(args)
                .filter(|(_, arg)| self.variables.contains(&arg))
                .map(|(v, _)| v)
                .collect_vec(),
        }
    }
    /// Delete unused code and arguments.
    fn remove_redundancy(
        &mut self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        let statements = statements
            .into_iter()
            .flat_map(|statement| match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    if self.variables.contains(&register) {
                        if let IntermediateExpression::IntermediateLambda(IntermediateLambda {
                            args,
                            block: IntermediateBlock { ret, statements },
                        }) = expression.clone()
                        {
                            let used_args = self.filter_args(&register, args.clone());
                            if used_args.len() != args.len() {
                                // Define an unoptimized variant of the function, which takes all the args and passes the relevant ones to the optimized variant.
                                let fresh_args = args
                                    .iter()
                                    .map(|arg| IntermediateArg::from(arg.type_.clone()))
                                    .collect_vec();
                                let fn_mem = IntermediateMemory::from(IntermediateType::from(
                                    IntermediateLambda {
                                        args: used_args.clone(),
                                        block: IntermediateBlock {
                                            statements: statements.clone(),
                                            ret: ret.clone(),
                                        },
                                    }
                                    .type_(),
                                ));
                                let ret_mem = IntermediateMemory::from(ret.type_());
                                self.variables.insert(fn_mem.register.clone());
                                self.variables.insert(ret_mem.register.clone());
                                self.fn_updates
                                    .insert(register.clone(), fn_mem.register.clone());
                                let unoptimized_fn = IntermediateLambda {
                                    args: fresh_args.clone(),
                                    block: IntermediateBlock {
                                        statements: vec![IntermediateAssignment {
                                            register: ret_mem.register.clone(),
                                            expression: IntermediateFnCall {
                                                fn_: fn_mem.clone().into(),
                                                args: self.filter_args(
                                                    &register,
                                                    fresh_args
                                                        .into_iter()
                                                        .map(Into::into)
                                                        .collect_vec(),
                                                ),
                                            }
                                            .into(),
                                        }
                                        .into()],
                                        ret: ret_mem.into(),
                                    },
                                }
                                .into();
                                return vec![
                                    IntermediateAssignment {
                                        expression: IntermediateLambda {
                                            args: used_args,
                                            block: IntermediateBlock { ret, statements },
                                        }
                                        .into(),
                                        register: fn_mem.register,
                                    }
                                    .into(),
                                    IntermediateAssignment {
                                        expression: unoptimized_fn,
                                        register,
                                    }
                                    .into(),
                                ];
                            }
                        }
                    }
                    vec![IntermediateAssignment {
                        expression,
                        register,
                    }
                    .into()]
                }
            })
            .collect_vec();
        statements
            .into_iter()
            .filter_map(|statement| match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    if self.variables.contains(&register) {
                        // Keep only used assignments.
                        Some(
                            IntermediateAssignment {
                                register: register.clone(),
                                expression: match expression {
                                    IntermediateExpression::IntermediateLambda(IntermediateLambda {
                                        args,
                                        block: IntermediateBlock { statements, ret },
                                    }) => IntermediateLambda {
                                        args,
                                        block: IntermediateBlock {
                                            statements: self.remove_redundancy(statements),
                                            ret,
                                        },
                                    }
                                    .into(),
                                    IntermediateExpression::IntermediateFnCall(
                                        IntermediateFnCall {
                                            fn_: IntermediateValue::IntermediateMemory(memory),
                                            args,
                                        },
                                    ) if self.fn_updates.contains_key(&memory.register)
                                        && !self.fn_updates.values().contains(&register) =>
                                    {
                                        // Update fn type if the fn is replaced with an optimized version.
                                        let IntermediateType::IntermediateFnType(
                                            IntermediateFnType(_, ret_type),
                                        ) = memory.type_
                                        else {
                                            panic!("Calling non-fn")
                                        };
                                        let args = self.filter_args(&memory.register, args);
                                        let type_ = IntermediateFnType(
                                            args.iter().map(|arg| arg.type_()).collect(),
                                            ret_type.clone(),
                                        )
                                        .into();
                                        IntermediateFnCall {
                                            args,
                                            fn_: IntermediateMemory {
                                                type_,
                                                register: self.fn_updates[&memory.register].clone(),
                                            }
                                            .into(),
                                        }
                                        .into()
                                    }
                                    IntermediateExpression::IntermediateIf(IntermediateIf {
                                        condition,
                                        branches,
                                    }) => IntermediateIf {
                                        condition,
                                        branches: (
                                            (
                                                self.remove_redundancy(branches.0.statements),
                                                branches.0.ret,
                                            )
                                                .into(),
                                            (
                                                self.remove_redundancy(branches.1.statements),
                                                branches.1.ret,
                                            )
                                                .into(),
                                        ),
                                    }
                                    .into(),
                                    IntermediateExpression::IntermediateMatch(IntermediateMatch {
                                        subject,
                                        branches,
                                    }) => IntermediateMatch {
                                        subject,
                                        branches: branches.into_iter().map(
                                            |IntermediateMatchBranch { target, block : IntermediateBlock { statements, ret }}| {
                                                IntermediateMatchBranch {
                                                    target: target.filter(|IntermediateArg { type_: _, register }| self.variables.contains(register)),
                                                    block: IntermediateBlock {
                                                        statements: self
                                                            .remove_redundancy(statements),
                                                        ret,
                                                    },
                                                }
                                            },
                                        ).collect_vec(),
                                    }
                                    .into(),
                                    expression => expression,
                                },
                            }
                            .into(),
                        )
                    } else {
                        None
                    }
                }
            })
            .collect_vec()
    }
    pub fn remove_dead_code(program: IntermediateProgram) -> IntermediateProgram {
        let mut optimizer = DeadCodeAnalyzer::new();
        let IntermediateLambda {
            args,
            block: IntermediateBlock { statements, ret },
        } = program.main;
        optimizer.generate_constraints(&statements);
        let IntermediateValue::IntermediateMemory(IntermediateMemory { type_: _, register }) = &ret
        else {
            return IntermediateProgram {
                main: IntermediateLambda {
                    args,
                    block: IntermediateBlock {
                        statements: Vec::new(),
                        ret,
                    },
                },
                types: program.types,
            };
        };
        let initial_solution = vec![register.clone()];
        optimizer.variables = optimizer.solve_constraints(initial_solution);
        let statements = optimizer.remove_redundancy(statements);
        IntermediateProgram {
            main: IntermediateLambda {
                args,
                block: IntermediateBlock { statements, ret },
            },
            types: program.types,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, rc::Rc};

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, ExpressionEqualityChecker, Id, Integer,
        IntermediateArg, IntermediateBuiltIn, IntermediateCtorCall, IntermediateElementAccess,
        IntermediateFnCall, IntermediateFnType, IntermediateIf, IntermediateLambda,
        IntermediateMatch, IntermediateMatchBranch, IntermediateProgram, IntermediateStatement,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateUnionType, IntermediateValue,
    };
    use test_case::test_case;

    #[test_case(
        (
            IntermediateValue::from(
                IntermediateBuiltIn::from(Integer{
                    value: 8
                })
            ).into(),
            Vec::new(),
        );
        "integer"
    )]
    #[test_case(
        (
            IntermediateValue::from(
                IntermediateBuiltIn::from(Boolean{
                    value: false
                })
            ).into(),
            Vec::new(),
        );
        "boolean"
    )]
    #[test_case(
        (
            IntermediateValue::from(
                BuiltInFn(
                    Id::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                )
            ).into(),
            Vec::new(),
        );
        "built-in fn"
    )]
    #[test_case(
        {
            let register = Register::new();
            (
                IntermediateValue::from(
                    IntermediateMemory{
                        register: register.clone(),
                        type_: AtomicTypeEnum::INT.into()
                    }
                ).into(),
                vec![register],
            )
        };
        "memory register"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            (
                IntermediateValue::from(
                    arg.clone()
                ).into(),
                vec![arg.register],
            )
        };
        "arg"
    )]
    #[test_case(
        {
            let memory = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            (
                IntermediateTupleExpression(vec![
                    arg.clone().into(), memory.clone().into(), IntermediateBuiltIn::from(Integer{value: 7}).into()
                ]).into(),
                vec![memory.register, arg.register],
            )
        };
        "tuple"
    )]
    #[test_case(
        {
            let memory = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![
                AtomicTypeEnum::INT.into(),
                AtomicTypeEnum::BOOL.into(),
            ])));
            (
                IntermediateElementAccess{
                    value: memory.clone().into(),
                    idx: 1
                }.into(),
                vec![memory.register],
            )
        };
        "element access"
    )]
    fn test_find_used_values(expression_registers: (IntermediateExpression, Vec<Register>)) {
        let (expression, expected_registers) = expression_registers;
        let mut optimizer = DeadCodeAnalyzer::new();

        let expected: HashSet<_> = expected_registers.into_iter().collect();
        let registers = optimizer.find_used_values(&expression);
        assert_eq!(HashSet::from_iter(registers), expected);
    }
    #[test_case(
        (
            vec![
                IntermediateAssignment{
                    expression: IntermediateValue::from(
                        IntermediateBuiltIn::from(Integer{
                            value: 8
                        })
                    ).into(),
                    register: Register::new()
                }.into()
            ],
            Vec::new(),
            Vec::new(),
        );
        "no constraint assignment"
    )]
    #[test_case(
        {
            let var1 = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let var2 = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let tuple = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![
                AtomicTypeEnum::INT.into(),
                AtomicTypeEnum::BOOL.into(),
            ])));
            let res = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateTupleExpression(vec![
                            var1.clone().into(), var2.clone().into()
                        ]).into(),
                        register: tuple.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateElementAccess{
                            value: tuple.clone().into(),
                            idx: 0
                        }.into(),
                        register: res.register.clone()
                    }.into()
                ],
                vec![
                    (tuple.register.clone(), vec![var1.register.clone(), var2.register.clone()]),
                    (res.register, vec![tuple.register]),
                ],
                Vec::new(),
            )
        };
        "basic assignments"
    )]
    #[test_case(
        {
            let id = IntermediateMemory::from(IntermediateType::from(IntermediateFnType(vec![AtomicTypeEnum::INT.into()],Box::new(AtomicTypeEnum::INT.into()))));
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![arg.clone()],
                            block: IntermediateBlock{
                                statements: Vec::new(),
                                ret: arg.clone().into()
                            },
                        }.into(),
                        register: id.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateFnCall{
                            fn_: id.clone().into(),
                            args: vec![
                                x.clone().into()
                            ]
                        }.into(),
                        register: y.register.clone()
                    }.into()
                ],
                vec![
                    (y.register.clone(), vec![id.register.clone()]),
                    (id.register.clone(), vec![arg.register.clone()]),
                ],
                vec![
                    (
                        (y.register.clone(), arg.register.clone()),
                        vec![x.register.clone()]
                    )
                ]
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateFnCall{
                            fn_: BuiltInFn(
                                Id::from("*"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            ).into(),
                            args: vec![
                                x.clone().into(),
                                y.clone().into()
                            ]
                        }.into(),
                        register: z.register.clone()
                    }.into()
                ],
                vec![
                    (z.register, vec![x.register, y.register]),
                ],
                Vec::new()
            )
        };
        "built-in fn call"
    )]
    #[test_case(
        {
            let f = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateFnCall{
                            fn_: f.clone().into(),
                            args: vec![
                                x.clone().into(),
                            ]
                        }.into(),
                        register: y.register.clone()
                    }.into()
                ],
                vec![
                    (y.register, vec![f.register, x.register]),
                ],
                Vec::new()
            )
        };
        "argument fn call"
    )]
    #[test_case(
        {
            let fn_ = Register::new();
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone(), y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: z.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: IntermediateValue::from(
                                                BuiltInFn(
                                                    Id::from("+"),
                                                    IntermediateFnType(
                                                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                                        Box::new(AtomicTypeEnum::INT.into())
                                                    ).into()
                                                )
                                            ),
                                            args: vec![y.clone().into(), IntermediateBuiltIn::from(Integer{value: 9}).into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: x.clone().into()
                            },
                        }.into(),
                        register: fn_.clone()
                    }.into(),
                ],
                vec![
                    (fn_.clone(), vec![x.register.clone()]),
                    (z.register.clone(), vec![y.register.clone()]),
                ],
                Vec::new()
            )
        };
        "fn with statements"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: bar_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: foo_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: vec![y.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        register: bar.register.clone()
                    }.into(),
                ],
                vec![
                    (foo.register.clone(), vec![bar_call.register.clone()]),
                    (bar.register.clone(), vec![foo_call.register.clone()]),
                    (foo_call.register.clone(), vec![foo.register.clone()]),
                    (bar_call.register.clone(), vec![bar.register.clone()]),
                ],
                vec![
                    ((foo_call.register.clone(), x.register.clone()), vec![y.register.clone()]),
                    ((bar_call.register.clone(), y.register.clone()), vec![x.register.clone()]),
                ],
            )
        };
        "mutually recursive fns"
    )]
    #[test_case(
        {
            let f = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let g = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![arg.clone()],
                            block: IntermediateBlock{
                                statements: Vec::new(),
                                ret: arg.clone().into()
                            },
                        }.into(),
                        register: f.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateValue::from(
                            f.clone()
                        ).into(),
                        register: g.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateFnCall{
                            fn_: g.clone().into(),
                            args: vec![
                                x.clone().into()
                            ]
                        }.into(),
                        register: y.register.clone()
                    }.into()
                ],
                vec![
                    (y.register.clone(), vec![g.register.clone(), x.register.clone()]),
                    (f.register.clone(), vec![arg.register.clone()]),
                    (g.register.clone(), vec![f.register.clone()]),
                ],
                Vec::new()
            )
        };
        "reassigned fn"
    )]
    #[test_case(
        {
            let c = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        register: z.register.clone(),
                        expression: IntermediateIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: x.register.clone(),
                                            expression:
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        }.into(),
                                    ],
                                    IntermediateValue::from(x.clone()).into()
                                ).into(),
                                IntermediateValue::from(y.clone()).into()
                            )
                        }.into()
                    }.into()
                ],
                vec![(
                    z.register,
                    vec![c.register, x.register, y.register]
                )],
                Vec::new()
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let s = IntermediateMemory::from(IntermediateType::from(
                IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into())])
            ));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        register: z.register.clone(),
                        expression: IntermediateMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(x.clone()),
                                    block: IntermediateValue::from(x.clone()).into()
                                },
                                IntermediateMatchBranch{
                                    target: Some(y.clone()),
                                    block: IntermediateValue::from(
                                        IntermediateBuiltIn::from(Integer{value: 0})
                                    ).into()
                                },
                            ]
                        }.into()
                    }.into()
                ],
                vec![
                    (z.register.clone(), vec![x.register, s.register]),
                ],
                Vec::new()
            )
        };
        "match statement"
    )]
    fn test_constraint_generation(
        statements_singles_doubles: (
            Vec<IntermediateStatement>,
            Vec<(Register, Vec<Register>)>,
            Vec<((Register, Register), Vec<Register>)>,
        ),
    ) {
        let (statements, expected_single_constraints, expected_double_constraints) =
            statements_singles_doubles;
        let mut optimizer = DeadCodeAnalyzer::new();

        optimizer.generate_constraints(&statements);

        let expected_single_constraints = HashMap::from_iter(
            expected_single_constraints
                .into_iter()
                .map(|(k, v)| (k, HashSet::from_iter(v))),
        );
        let expected_double_constraints = HashMap::from_iter(
            expected_double_constraints
                .into_iter()
                .map(|((reg1, reg2), v)| (minmax(reg1, reg2).into(), HashSet::from_iter(v))),
        );
        let single_constraints: HashMap<_, _> = optimizer
            .single_constraints
            .into_iter()
            .filter_map(|(k, v)| if v.len() > 0 { Some((k, v)) } else { None })
            .collect();
        assert_eq!(single_constraints, expected_single_constraints);
        assert_eq!(optimizer.double_constraints, expected_double_constraints);
    }

    #[test_case(
        {
            let register = Register::new();
            (
                register.clone(),
                (Vec::new(), Vec::new()),
                vec![register]
            )
        };
        "no constraints"
    )]
    #[test_case(
        {
            let a = Register::new();
            let b = Register::new();
            let c = Register::new();
            let d = Register::new();
            let e = Register::new();
            let f = Register::new();
            (
                a.clone(),
                (
                    vec![
                        (a.clone(), vec![b.clone()]),
                        (b.clone(), vec![a.clone(), c.clone()]),
                        (c.clone(), vec![e.clone()]),
                        (d.clone(), vec![e.clone()]),
                        (e.clone(), Vec::new()),
                        (f.clone(), vec![d.clone()]),
                    ],
                    Vec::new()
                ),
                vec![a,b,c,e]
            )
        };
        "single constraints only"
    )]
    #[test_case(
        {
            let a = Register::new();
            let b = Register::new();
            let c = Register::new();
            let d = Register::new();
            let e = Register::new();
            let f = Register::new();
            (
                a.clone(),
                (
                    vec![
                        (a.clone(), vec![b.clone()]),
                        (e.clone(), vec![d.clone()]),
                    ],
                    vec![
                        ((a.clone(), b.clone()), vec![c.clone()]),
                        ((b.clone(), c.clone()), vec![a.clone(),f.clone()]),
                        ((a.clone(), d.clone()), vec![e.clone()]),
                    ]
                ),
                vec![a,b,c,f]
            )
        };
        "mixed constraints"
    )]
    fn test_solving_constraints(
        initial_constraints_solution: (
            Register,
            (
                Vec<(Register, Vec<Register>)>,
                Vec<((Register, Register), Vec<Register>)>,
            ),
            Vec<Register>,
        ),
    ) {
        let (initial_solution, (single_constraints, double_constraints), expected_solution) =
            initial_constraints_solution;
        let mut optimizer = DeadCodeAnalyzer::new();
        for (k, v) in single_constraints {
            optimizer.add_single_constraint(k, v);
        }
        for ((r1, r2), v) in double_constraints {
            optimizer.add_double_constraint(r1, r2, v);
        }
        let solution = optimizer.solve_constraints(vec![initial_solution]);
        assert_eq!(solution, HashSet::from_iter(expected_solution));
    }

    #[test_case(
        {
            let w = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            let unused = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock {
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: unused.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: false})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: x.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: w.register.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: -1})).into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: y.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                fn_: BuiltInFn(
                                                                    Id::from("--"),
                                                                    IntermediateFnType(
                                                                        vec![AtomicTypeEnum::INT.into()],
                                                                        Box::new(AtomicTypeEnum::INT.into())
                                                                    ).into()
                                                                ).into(),
                                                                args: vec![
                                                                    x.clone().into()
                                                                ]
                                                            }.into(),
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: z.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                fn_: BuiltInFn(
                                                                    Id::from("++"),
                                                                    IntermediateFnType(
                                                                        vec![AtomicTypeEnum::INT.into()],
                                                                        Box::new(AtomicTypeEnum::INT.into())
                                                                    ).into()
                                                                ).into(),
                                                                args: vec![
                                                                    w.clone().into()
                                                                ]
                                                            }.into(),
                                                    }.into(),
                                                ],
                                                ret: y.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new(),
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: x.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: y.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                fn_: BuiltInFn(
                                                                    Id::from("--"),
                                                                    IntermediateFnType(
                                                                        vec![AtomicTypeEnum::INT.into()],
                                                                        Box::new(AtomicTypeEnum::INT.into())
                                                                    ).into()
                                                                ).into(),
                                                                args: vec![
                                                                    x.clone().into()
                                                                ]
                                                            }.into(),
                                                    }.into(),
                                                ],
                                                ret: y.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new(),
                },
            )
        };
        "unused variables"
    )]
    #[test_case(
        {
            let opt_main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ))
            );
            let opt_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::BOOL)
            );
            let un_opt_main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::BOOL.into(),
                    ],
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ))
            );
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::BOOL.into(),
                    ],
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ))
            );
            let args = vec![
                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)),
                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL)),
            ];
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: args.clone(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![
                                                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)),
                                                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL))
                                            ],
                                            block: IntermediateBlock{
                                                statements: Vec::new(),
                                                ret: Boolean{value: true}.into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: args.iter().cloned().map(IntermediateValue::from).collect()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new(),
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: args.clone(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: opt_main.register.clone(),
                                    expression: IntermediateLambda{
                                        args: Vec::new(),
                                        block: IntermediateBlock{
                                            statements: Vec::new(),
                                            ret: Boolean{value: true}.into()
                                        },
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: un_opt_main.register.clone(),
                                    expression: IntermediateLambda{
                                        args: vec![
                                            IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)),
                                            IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL))
                                        ],
                                        block: IntermediateBlock {
                                            statements: vec![
                                                IntermediateAssignment{
                                                    register: opt_call.register.clone(),
                                                    expression: IntermediateFnCall{
                                                        fn_: opt_main.clone().into(),
                                                        args: Vec::new()
                                                    }.into()
                                                }.into()
                                            ],
                                            ret: opt_call.clone().into()
                                        },
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: opt_main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ]
                        },
                    },
                    types: Vec::new(),
                },
            )
        };
        "unused main args"
    )]
    #[test_case(
        {
            let c = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let w = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: c.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: z.register.clone(),
                                    expression: IntermediateIf{
                                        condition: c.clone().into(),
                                        branches: (
                                            (
                                                vec![
                                                    IntermediateAssignment{
                                                        register: x.register.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                                    }.into(),
                                                ],
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            ).into(),
                                            (
                                                vec![
                                                    IntermediateAssignment{
                                                        register: y.register.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: w.register.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 7})).into()
                                                    }.into(),
                                                ],
                                                IntermediateValue::from(y.clone()).into()
                                            ).into()
                                        )
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: Vec::new(),
                                                ret: z.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: c.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: z.register.clone(),
                                    expression: IntermediateIf{
                                        condition: c.clone().into(),
                                        branches: (
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into(),
                                            (
                                                vec![
                                                    IntermediateAssignment{
                                                        register: y.register.clone(),
                                                        expression: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()
                                                    }.into(),
                                                ],
                                                IntermediateValue::from(y.clone()).into()
                                            ).into()
                                        )
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock{
                                                statements: Vec::new(),
                                                ret: z.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                }
            )
        };
        "unused in if statement"
    )]
    #[test_case(
        {
            let c = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: c.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: y.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 2})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: x.register.clone(),
                                    expression: IntermediateIf{
                                        condition: c.clone().into(),
                                        branches: (
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into(),
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                        )
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock{
                                                statements: Vec::new(),
                                                ret: y.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        }.into()
                    },
                    types: Vec::new()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: y.register.clone(),
                                    expression:
                                        IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 2})).into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression: IntermediateLambda{
                                        args: Vec::new(),
                                        block: IntermediateBlock{
                                            statements: Vec::new(),
                                            ret: y.clone().into(),
                                        },
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
            )
        };
        "unused function argument"
    )]
    #[test_case(
        {
            let s = IntermediateMemory::from(IntermediateType::from(
                IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into())])
            ));
            let w = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),]).into()))
            ];
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: s.register.clone(),
                                    expression: IntermediateCtorCall{
                                        idx: 0,
                                        data: None,
                                        type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: z.register.clone(),
                                    expression: IntermediateMatch{
                                        subject: s.clone().into(),
                                        branches: vec![
                                            IntermediateMatchBranch {
                                                target: Some(
                                                    IntermediateArg {
                                                        type_: AtomicTypeEnum::INT.into(),
                                                        register: Register::new()
                                                    }
                                                ),
                                                block: IntermediateBlock{
                                                    statements: vec![
                                                        IntermediateAssignment{
                                                            register: x.register.clone(),
                                                            expression: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                                        }.into(),
                                                    ],
                                                    ret: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                                }
                                            },
                                            IntermediateMatchBranch {
                                                target: Some(IntermediateArg {
                                                    type_: AtomicTypeEnum::INT.into(),
                                                    register: y.register.clone()
                                                }),
                                                block: IntermediateBlock {
                                                    statements: vec![
                                                        IntermediateAssignment{
                                                            register: w.register.clone(),
                                                            expression:
                                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 7})).into()
                                                        }.into(),
                                                    ],
                                                    ret: IntermediateValue::from(y.clone()).into()
                                                }
                                            }
                                        ],
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression: IntermediateLambda{
                                        args: Vec::new(),
                                        block: IntermediateBlock {
                                            statements: Vec::new(),
                                            ret: z.clone().into(),
                                        },
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: types.clone()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: s.register.clone(),
                                    expression: IntermediateCtorCall{
                                        idx: 0,
                                        data: None,
                                        type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                    }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: z.register.clone(),
                                    expression: IntermediateMatch{
                                        subject: s.clone().into(),
                                        branches: vec![
                                            IntermediateMatchBranch {
                                                target: None,
                                                block: IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            },
                                            IntermediateMatchBranch {
                                                target: Some(IntermediateArg {
                                                    type_: AtomicTypeEnum::INT.into(),
                                                    register: y.register.clone()
                                                }),
                                                block: IntermediateValue::from(y.clone()).into()
                                            }
                                        ],
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: Vec::new(),
                                                ret: z.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: types.clone()
                },
            )
        };
        "unused in match"
    )]
    #[test_case(
        {
            let s = IntermediateMemory::from(IntermediateType::from(
                IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into())])
            ));
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),]).into()))
            ];
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: s.register.clone(),
                                    expression:
                                        IntermediateCtorCall{
                                            idx: 0,
                                            data: None,
                                            type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: z.register.clone(),
                                    expression: IntermediateMatch {
                                        subject: s.clone().into(),
                                        branches: vec![
                                            IntermediateMatchBranch {
                                                target: Some(
                                                    IntermediateArg {
                                                        type_: AtomicTypeEnum::INT.into(),
                                                        register: x.register.clone()
                                                    }
                                                ),
                                                block: IntermediateValue::from(x.clone()).into(),
                                            },
                                            IntermediateMatchBranch {
                                                target: Some(IntermediateArg {
                                                    type_: AtomicTypeEnum::INT.into(),
                                                    register: y.register.clone()
                                                }),
                                                block: IntermediateValue::from(y.clone()).into(),
                                            }
                                        ],
                                    }.into(),
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: Vec::new(),
                                                ret: IntermediateBuiltIn::from(Integer{value: -8}).into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: types.clone()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: Vec::new(),
                                                ret: IntermediateBuiltIn::from(Integer{value: -8}).into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: types.clone()
                },
            )
        };
        "unused match statement"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_opt = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_opt_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let apply = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![
                        IntermediateType::from(IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ))
                    ],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let apply_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let f_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let f = IntermediateArg::from(IntermediateType::from(IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let tuple = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![
                AtomicTypeEnum::INT.into(),
                AtomicTypeEnum::INT.into(),
            ])));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: foo.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![arg.clone()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![arg.clone().into()],
                                                                fn_: foo.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: apply.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![f.clone(), x.clone()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: f_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![x.clone().into()],
                                                                fn_: f.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: f_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_main_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![IntermediateBuiltIn::from(Integer{value: 3}).into()],
                                                                fn_: foo.clone().into()
                                                            }.into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: apply_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![
                                                                    foo.clone().into(),
                                                                    IntermediateBuiltIn::from(Integer{value: 3}).into()
                                                                ],
                                                                fn_: apply.clone().into()
                                                            }.into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: tuple.register.clone(),
                                                        expression:
                                                            IntermediateTupleExpression(vec![
                                                                foo_main_call.clone().into(),
                                                                apply_call.clone().into(),
                                                            ]).into()
                                                    }.into(),
                                                ],
                                                ret: tuple.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: main_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: foo_opt.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: foo.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![arg.clone().into()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_opt_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_opt_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: apply.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![f.clone(), x.clone()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: f_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![x.clone().into()],
                                                                fn_: f.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: f_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_main_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: apply_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![
                                                                    foo.clone().into(),
                                                                    IntermediateBuiltIn::from(Integer{value: 3}).into()
                                                                ],
                                                                fn_: apply.clone().into()
                                                            }.into()
                                                    }.into(),
                                                    IntermediateAssignment{
                                                        register: tuple.register.clone(),
                                                        expression:
                                                            IntermediateTupleExpression(vec![
                                                                foo_main_call.clone().into(),
                                                                apply_call.clone().into(),
                                                            ]).into()
                                                    }.into(),
                                                ],
                                                ret: tuple.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
            )
        };
        "unused argument"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_opt = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_un_opt_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_opt = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_un_opt_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let bar_arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let last_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: last_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: foo.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![foo_arg.clone()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![foo_arg.clone().into()],
                                                                fn_: bar.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: bar.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![bar_arg.clone()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: bar_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![bar_arg.clone().into()],
                                                                fn_: foo.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: bar_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: main_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: vec![IntermediateBuiltIn::from(Integer{value: 3}).into()],
                                                                fn_: foo.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: main_call.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: last_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock{
                            ret: last_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: foo_opt.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: bar_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: foo.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![foo_arg.clone().into()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: foo_un_opt_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: foo_un_opt_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: bar_opt.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: bar_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: bar_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: bar.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![bar_arg.clone().into()],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: bar_un_opt_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: bar_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: bar_un_opt_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: main_call.register.clone(),
                                                        expression:
                                                            IntermediateFnCall{
                                                                args: Vec::new(),
                                                                fn_: foo_opt.clone().into()
                                                            }.into()
                                                    }.into(),
                                                ],
                                                ret: main_call.clone().into(),
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: last_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new(),
                },
            )
        };
        "unused shared arguments"
    )]
    #[test_case(
        {
            let main = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_opt = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let main_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            let arg = IntermediateArg::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            let last_call = IntermediateMemory::from(
                IntermediateType::from(AtomicTypeEnum::INT)
            );
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: vec![arg.clone().into()],
                        block: IntermediateBlock{
                            ret: last_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![
                                                IntermediateArg::from(
                                                    IntermediateType::from(AtomicTypeEnum::INT)
                                                )
                                            ],
                                            block: IntermediateBlock{
                                                statements: Vec::new(),
                                                ret: Integer{value: 0}.into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: last_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main.clone().into(),
                                        args: vec![arg.clone().into()]
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: vec![arg.clone().into()],
                        block: IntermediateBlock{
                            ret: last_call.clone().into(),
                            statements: vec![
                                IntermediateAssignment{
                                    register: main_opt.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: Vec::new(),
                                            block: IntermediateBlock{
                                                statements: Vec::new(),
                                                ret: Integer{value: 0}.into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: main.register.clone(),
                                    expression:
                                        IntermediateLambda{
                                            args: vec![
                                                IntermediateArg::from(
                                                    IntermediateType::from(AtomicTypeEnum::INT)
                                                )
                                            ],
                                            block: IntermediateBlock {
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        register: main_call.register.clone(),
                                                        expression: IntermediateFnCall{
                                                            fn_: main_opt.clone().into(),
                                                            args: Vec::new()
                                                        }.into()
                                                    }.into(),
                                                ],
                                                ret: main_call.clone().into()
                                            },
                                        }.into()
                                }.into(),
                                IntermediateAssignment{
                                    register: last_call.register.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: main_opt.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into(),
                            ],
                        },
                    },
                    types: Vec::new(),
                },
            )
        };
        "unused main arg"
    )]
    fn test_remove_program_dead_code(program_expected: (IntermediateProgram, IntermediateProgram)) {
        let (program, expected_program) = program_expected;
        let optimized_program = DeadCodeAnalyzer::remove_dead_code(program);
        dbg!(&expected_program, &optimized_program);
        assert_eq!(optimized_program.types, expected_program.types);
        ExpressionEqualityChecker::assert_equal(
            &optimized_program.main.into(),
            &expected_program.main.into(),
        )
    }
}
