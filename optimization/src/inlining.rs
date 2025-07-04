use itertools::Itertools;
use lowering::{
    FnDefs, FnInst, IntermediateAssignment, IntermediateBlock, IntermediateExpression,
    IntermediateFnCall, IntermediateIf, IntermediateLambda, IntermediateMatch,
    IntermediateMatchBranch, IntermediateMemory, IntermediateProgram, IntermediateStatement,
    IntermediateValue,
};
use std::convert::identity;

use crate::{redundancy_elimination::RedundancyEliminator, refresher::Refresher};
use itertools::Either::{Left, Right};
use translation::CodeSizeEstimator;

pub struct Inliner {
    fn_defs: FnDefs,
    size_limit: usize,
}

// Define exit limit in case of fns that are repeatedly inlined but do not increase in size.
const MAX_INLINING_ITERATIONS: usize = 1000;

impl Inliner {
    pub fn inline_up_to_size(
        program: IntermediateProgram,
        size_limit: Option<usize>,
    ) -> IntermediateProgram {
        let mut should_continue = true;
        let mut program = program;
        let mut i = 0;
        while should_continue && i < MAX_INLINING_ITERATIONS {
            (program.main, should_continue) = Inliner::inline_iteration(program.main, size_limit);
            // Clean up with equivalent expression elimination after each iteration.
            program = RedundancyEliminator::eliminate_redundancy(program);
            i += 1;
        }
        program
    }
    fn new() -> Self {
        Inliner {
            fn_defs: FnDefs::new(),
            size_limit: usize::max_value(),
        }
    }

    /// Inline a function, generating statements and a value.
    fn inline(
        &self,
        mut lambda: IntermediateLambda,
        args: Vec<IntermediateValue>,
    ) -> IntermediateBlock {
        // Refresh the lambda to maintain SSA.
        Refresher::refresh_for_inlining(&mut lambda);
        let assignments = lambda
            .args
            .iter()
            .zip_eq(args.into_iter())
            .map(|(arg, v)| {
                IntermediateAssignment {
                    register: arg.register.clone(),
                    expression: v.into(),
                }
                .into()
            })
            .collect_vec();
        let mut statements = assignments;
        statements.extend(lambda.block.statements);
        (statements, lambda.block.ret).into()
    }

    fn inline_iteration(
        lambda: IntermediateLambda,
        size_limit: Option<usize>,
    ) -> (IntermediateLambda, bool) {
        // If the lambda is already too big, do nothing.
        let bounds = CodeSizeEstimator::estimate_size(&lambda);
        if let Some(size) = size_limit {
            if bounds.1 >= size {
                return (lambda, false);
            }
        }
        let IntermediateLambda {
            args,
            block: IntermediateBlock { statements, ret },
        } = lambda;
        // Register statements and set size limit.
        let mut inliner = Inliner::from(&statements);
        if let Some(size) = size_limit {
            inliner.size_limit = size;
        }
        let inliner = inliner;
        // Inline statements that are below a certain size.
        let (statements, should_continue) = inliner.inline_statements(statements);
        (
            IntermediateLambda {
                args,
                block: IntermediateBlock { statements, ret },
            },
            should_continue,
        )
    }
    fn inline_statements(
        &self,
        statements: Vec<IntermediateStatement>,
    ) -> (Vec<IntermediateStatement>, bool) {
        let (statements, continues): (Vec<_>, Vec<_>) = statements
            .into_iter()
            .map(|statement| self.inline_statement(statement))
            .unzip();
        (statements.concat(), continues.into_iter().any(identity))
    }
    fn inline_statement(
        &self,
        statement: IntermediateStatement,
    ) -> (Vec<IntermediateStatement>, bool) {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                self.inline_assignment(assignment)
            }
        }
    }
    fn inline_assignment(
        &self,
        IntermediateAssignment {
            expression,
            register,
        }: IntermediateAssignment,
    ) -> (Vec<IntermediateStatement>, bool) {
        let mut should_continue = false;
        let mut statements = Vec::new();
        let expression = match expression {
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                fn_: IntermediateValue::IntermediateMemory(IntermediateMemory { type_, register }),
                args,
            }) if self.fn_defs.contains_key(&register) => {
                match FnInst::get_root_fn(&self.fn_defs, &register) {
                    Some(Left(lambda))
                        if CodeSizeEstimator::estimate_size(&lambda).1 < self.size_limit =>
                    {
                        let IntermediateBlock {
                            statements: extra_statements,
                            ret: value,
                        } = self.inline(lambda.clone(), args);
                        statements = extra_statements;
                        should_continue = true;
                        value.into()
                    }
                    Some(Right(built_in_fn)) => IntermediateFnCall {
                        fn_: built_in_fn.clone().into(),
                        args,
                    }
                    .into(),
                    _ => IntermediateFnCall {
                        fn_: IntermediateMemory { type_, register }.into(),
                        args,
                    }
                    .into(),
                }
            }
            IntermediateExpression::IntermediateLambda(lambda)
                if CodeSizeEstimator::estimate_size(&lambda).1 < self.size_limit =>
            {
                // Inline lambda if it is below the size limit.
                let IntermediateLambda {
                    args,
                    block: IntermediateBlock { statements, ret },
                } = lambda;
                let (statements, internal_continue) = self.inline_statements(statements);
                should_continue |= internal_continue;
                IntermediateLambda {
                    args,
                    block: IntermediateBlock { statements, ret },
                }
                .into()
            }
            IntermediateExpression::IntermediateIf(IntermediateIf {
                condition,
                branches,
            }) => {
                let (statements_0, continue_0) = self.inline_statements(branches.0.statements);
                should_continue |= continue_0;
                let (statements_1, continue_1) = self.inline_statements(branches.1.statements);
                should_continue |= continue_1;
                IntermediateIf {
                    condition,
                    branches: (
                        (statements_0, branches.0.ret).into(),
                        (statements_1, branches.1.ret).into(),
                    ),
                }
                .into()
            }
            IntermediateExpression::IntermediateMatch(IntermediateMatch { subject, branches }) => {
                let branches = branches
                    .into_iter()
                    .map(
                        |IntermediateMatchBranch {
                             target,
                             block: IntermediateBlock { statements, ret },
                         }| {
                            let (statements, internal_continue) =
                                self.inline_statements(statements);
                            should_continue |= internal_continue;
                            IntermediateMatchBranch {
                                target,
                                block: IntermediateBlock { statements, ret },
                            }
                        },
                    )
                    .collect();
                IntermediateMatch { subject, branches }.into()
            }
            _ => expression,
        };
        statements.push(
            IntermediateAssignment {
                expression,
                register,
            }
            .into(),
        );
        (statements, should_continue)
    }
}

/// Inliner::from(statements)
impl From<&Vec<IntermediateStatement>> for Inliner {
    fn from(statements: &Vec<IntermediateStatement>) -> Self {
        let mut inliner = Inliner::new();
        FnInst::collect_fn_defs_from_statements(statements, &mut inliner.fn_defs);
        inliner
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, collections::HashSet, rc::Rc};

    use super::*;
    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, ExpressionEqualityChecker, Id, Integer,
        IntermediateArg, IntermediateAssignment, IntermediateBuiltIn, IntermediateFnCall,
        IntermediateFnType, IntermediateIf, IntermediateMatch, IntermediateMatchBranch,
        IntermediateMemory, IntermediateType, IntermediateUnionType, IntermediateValue, Register,
    };
    use test_case::test_case;

    #[test_case(
        (
            IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            },
            Vec::new(),
            (
                Vec::new(),
                Integer{value: 11}.into(),
            )
        );
        "trivial fn"
    )]
    #[test_case(
        {
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let value = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    block: IntermediateBlock{
                        statements: Vec::new(),
                        ret: arg.clone().into()
                    },
                },
                vec![Integer{value: 22}.into()],
                (
                    vec![
                        IntermediateAssignment{
                            register: value.register.clone(),
                            expression: IntermediateValue::from(Integer{value: 22}).into()
                        }.into()
                    ],
                    value.clone().into()
                )
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    register: Register::new(),
                },
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    register: Register::new(),
                },
            ];
            let mem = args.iter().map(|arg| IntermediateMemory {
                register: Register::new(),
                type_: arg.type_.clone()
            }).collect_vec();
            let ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            (
                IntermediateLambda {
                    args: args.clone(),
                    block: IntermediateBlock {
                        statements: vec![
                            IntermediateAssignment {
                                expression: IntermediateFnCall {
                                    fn_: IntermediateBuiltIn::from(BuiltInFn(
                                        Id::from("+"),
                                        IntermediateFnType(
                                            vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                            Box::new(AtomicTypeEnum::INT.into()),
                                        )
                                    )).into(),
                                    args: args.clone().into_iter().map(|arg| arg.into()).collect_vec(),
                                }.into(),
                                register: ret.register.clone()
                            }.into()
                        ],
                        ret: ret.clone().into()
                    },
                },
                vec![Integer{value: 11}.into(), Integer{value: -11}.into()],
                (
                    vec![
                        IntermediateAssignment {
                            expression: IntermediateValue::from(Integer{value: 11}).into(),
                            register: mem[0].register.clone()
                        }.into(),
                        IntermediateAssignment {
                            expression: IntermediateValue::from(Integer{value: -11}).into(),
                            register: mem[1].register.clone()
                        }.into(),
                        IntermediateAssignment {
                            expression: IntermediateFnCall {
                                fn_: IntermediateBuiltIn::from(BuiltInFn(
                                    Id::from("+"),
                                    IntermediateFnType(
                                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::INT.into()),
                                    )
                                )).into(),
                                args: mem.clone().into_iter().map(|mem| mem.into()).collect_vec(),
                            }.into(),
                            register: ret.register.clone()
                        }.into()
                    ],
                    ret.clone().into()
                )
            )
        };
        "plus fn"
    )]
    fn test_inline_fn(
        lambda_args_expected: (
            IntermediateLambda,
            Vec<IntermediateValue>,
            (Vec<IntermediateStatement>, IntermediateValue),
        ),
    ) {
        let (lambda, args, expected) = lambda_args_expected;
        let inliner = Inliner::new();
        let mut fn_targets = IntermediateStatement::all_targets(&lambda.block.statements);
        fn_targets.extend(lambda.args.iter().map(|arg| arg.register.clone()));
        let result = inliner.inline(lambda, args);
        let targets = IntermediateStatement::all_targets(&result.statements);

        dbg!(&expected, &result);
        ExpressionEqualityChecker::assert_equal(
            &IntermediateLambda {
                args: Vec::new(),
                block: result,
            }
            .into(),
            &IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements: expected.0,
                    ret: expected.1,
                },
            }
            .into(),
        );
        assert!(HashSet::<Register>::from_iter(fn_targets)
            .intersection(&HashSet::from_iter(targets))
            .collect_vec()
            .is_empty())
    }

    #[test]
    fn test_fn_refresh() {
        let id_arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let id = IntermediateLambda {
            args: vec![id_arg.clone()],
            block: IntermediateBlock {
                statements: Vec::new(),
                ret: id_arg.clone().into(),
            },
        };
        let id_fn = IntermediateMemory {
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
            register: Register::new(),
        };

        let idea_arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let idea_ret = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let idea = IntermediateLambda {
            args: vec![idea_arg.clone()],
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        register: id_fn.register.clone(),
                        expression: id.clone().into(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: idea_ret.register.clone(),
                        expression: IntermediateFnCall {
                            fn_: id_fn.clone().into(),
                            args: vec![idea_arg.clone().into()],
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: idea_ret.clone().into(),
            },
        };

        let inliner = Inliner::new();
        let result = inliner.inline(idea, vec![Integer { value: 0 }.into()]);
        let expected = (
            vec![
                IntermediateAssignment {
                    register: idea_arg.register.clone(),
                    expression: IntermediateValue::from(Integer { value: 0 }).into(),
                }
                .into(),
                IntermediateAssignment {
                    register: id_fn.register.clone(),
                    expression: id.clone().into(),
                }
                .into(),
                IntermediateAssignment {
                    register: idea_ret.register.clone(),
                    expression: IntermediateFnCall {
                        fn_: id_fn.clone().into(),
                        args: vec![IntermediateMemory {
                            type_: idea_arg.type_.clone(),
                            register: idea_arg.register.clone(),
                        }
                        .into()],
                    }
                    .into(),
                }
                .into(),
            ],
            idea_ret.clone().into(),
        );

        dbg!(&expected, &result);
        ExpressionEqualityChecker::assert_equal(
            &IntermediateLambda {
                args: Vec::new(),
                block: result.clone(),
            }
            .into(),
            &IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements: expected.0,
                    ret: expected.1,
                },
            }
            .into(),
        );

        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IntermediateLambda(IntermediateLambda { args, block: _ }),
            register: _,
        }) = &result.statements[1]
        else {
            panic!()
        };
        assert_ne!(args, &vec![id_arg]);
    }

    #[test_case(
        {
            let fn_ = IntermediateMemory{
                register: Register::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let ret_register = Register::new();
            let lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 1}.into()
                },
            };
            (
                vec![
                    IntermediateAssignment {
                        register: fn_.register.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: ret_register.clone(),
                        expression: IntermediateFnCall{
                            fn_: fn_.clone().into(),
                            args: Vec::new()
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        register: fn_.register.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateValue::from(Integer{value: 1}).into()
                    }.into(),
                ]
            )
        },
        true;
        "trivial fn"
    )]
    #[test_case(
        {
            let fn_ = IntermediateMemory{
                register: Register::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let ret_register = Register::new();
            let op = IntermediateValue::from(BuiltInFn(
                Id::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            (
                vec![
                    IntermediateAssignment {
                        register: fn_.register.clone(),
                        expression: op.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: ret_register.clone(),
                        expression: IntermediateFnCall{
                            fn_: fn_.clone().into(),
                            args: vec![Integer{value: 3}.into()]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        register: fn_.register.clone(),
                        expression: op.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: ret_register.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone(),
                            args: vec![Integer{value: 3}.into()]
                        }.into()
                    }.into(),
                ]
            )
        },
        false;
        "built-in fn"
    )]
    #[test_case(
        {
            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: id_arg.clone().into(),
                },
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            register: id_fn.register.clone(),
                            expression: id.clone().into(),
                        }
                        .into(),
                        IntermediateAssignment {
                            register: idea_ret.register.clone(),
                            expression: IntermediateFnCall {
                                fn_: id_fn.clone().into(),
                                args: vec![idea_arg.clone().into()],
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };
            let ret = Register::new();

            let inner_arg = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let outer_res = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let outer_arg = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let fresh_id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            (
                vec![
                    IntermediateAssignment{
                        register: idea_fn.register.clone(),
                        expression: idea.clone().into()
                    }.into(),
                    IntermediateAssignment{
                        register: ret.clone(),
                        expression: IntermediateFnCall{
                            fn_: idea_fn.clone().into(),
                            args: vec![Integer{value: 5}.into()]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        register: idea_fn.register.clone(),
                        expression: IntermediateLambda {
                            args: vec![idea_arg.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment {
                                        register: Register::new(),
                                        expression: id.clone().into(),
                                    }.into(),
                                    IntermediateAssignment{
                                        register: inner_arg.register.clone(),
                                        expression: IntermediateValue::from(
                                            idea_arg.clone()
                                        ).into()
                                    }.into(),
                                    IntermediateAssignment{
                                        register: idea_ret.register.clone(),
                                        expression: IntermediateValue::from(
                                            inner_arg.clone()
                                        ).into()
                                    }.into(),
                                ],
                                ret: idea_ret.clone().into(),
                            },
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        register: outer_arg.register.clone(),
                        expression: IntermediateValue::from(
                            Integer{value: 5}
                        ).into()
                    }.into(),
                    IntermediateAssignment{
                        register: id_fn.register.clone(),
                        expression: IntermediateLambda {
                            args: vec![fresh_id_arg.clone()],
                            block: IntermediateBlock{
                                statements: Vec::new(),
                                ret: fresh_id_arg.clone().into(),
                            },
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        register: outer_res.register.clone(),
                        expression: IntermediateFnCall{
                            fn_: id_fn.clone().into(),
                            args: vec![outer_arg.clone().into()]
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        register: ret.clone(),
                        expression: IntermediateValue::from(
                            outer_res
                        ).into()
                    }.into(),
                ],
            )
        },
        true;
        "nested fn"
    )]
    #[test_case(
        {
            let inc = IntermediateValue::from(BuiltInFn(
                Id::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            let dec = IntermediateValue::from(BuiltInFn(
                Id::from("--"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            let op = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };
            let t0 = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };
            let t1 = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };

            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: id_arg.clone().into(),
                },
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                register: Register::new(),
            };
            let extra = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };

            let ret_register = Register::new();
            let condition = IntermediateArg{
                type_: AtomicTypeEnum::BOOL.into(),
                register: Register::new()
            };
            (
                vec![
                    IntermediateAssignment {
                        register: op.register.clone(),
                        expression: IntermediateIf {
                            condition: condition.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment {
                                            register: id_fn.register.clone(),
                                            expression: id.clone().into()
                                        }.into(),
                                        IntermediateAssignment {
                                            register: Register::new(),
                                            expression: IntermediateFnCall{
                                                fn_: id_fn.clone().into(),
                                                args: vec![
                                                    IntermediateValue::from(Integer{value: -7}).into()
                                                ]
                                            }.into()
                                        }.into(),
                                        IntermediateAssignment {
                                            register: t0.register.clone(),
                                            expression: inc.clone().into()
                                        }.into(),
                                    ],
                                    t0.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment {
                                            register: t1.register.clone(),
                                            expression: dec.clone().into()
                                        }.into(),
                                    ],
                                    t1.clone().into()
                                ).into()
                            )
                        }.into(),
                    }.into(),
                    IntermediateAssignment {
                        register: ret_register.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone().into(),
                            args: vec![
                                IntermediateValue::from(Integer{value: -8}).into()
                            ]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        register: op.register.clone(),
                        expression: IntermediateIf {
                            condition: condition.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment {
                                            register: Register::new(),
                                            expression: id.clone().into(),
                                        }.into(),
                                        IntermediateAssignment{
                                            register: extra.register.clone(),
                                            expression: IntermediateValue::from(
                                                Integer{value: -7}
                                            ).into()
                                        }.into(),
                                        IntermediateAssignment{
                                            register: Register::new(),
                                            expression: IntermediateValue::from(
                                                extra.clone()
                                            ).into()
                                        }.into(),
                                        IntermediateAssignment {
                                            register: t0.register.clone(),
                                            expression: inc.clone().into()
                                        }.into(),
                                    ],
                                    t0.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment {
                                            register: t1.register.clone(),
                                            expression: dec.clone().into()
                                        }.into(),
                                    ],
                                    t1.clone().into()
                                ).into()
                            )
                        }.into(),
                    }.into(),
                    IntermediateAssignment {
                        register: ret_register.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone().into(),
                            args: vec![
                                IntermediateValue::from(Integer{value: -8}).into()
                            ]
                        }.into()
                    }.into(),
                ]
            )
        },
        true;
        "if statement"
    )]
    #[test_case(
        {
            let lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Boolean{value: false}.into(),
                },
            };
            let memory = IntermediateMemory {
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::BOOL.into())
                ).into(),
                register: Register::new(),
            };
            let target = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::BOOL));

            let subject = IntermediateArg{
                type_: IntermediateUnionType(vec![None]).into(),
                register: Register::new()
            };
            (
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                            expression: IntermediateMatch {
                            subject: subject.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment {
                                                register: memory.register.clone(),
                                                expression: lambda.clone().into()
                                            }.into(),
                                            IntermediateAssignment {
                                                register: target.register.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: memory.clone().into(),
                                                    args: Vec::new()
                                                }.into()
                                            }.into(),
                                        ],
                                        target.clone().into()
                                    ).into()
                                },
                            ],
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                            expression: IntermediateMatch {
                            subject: subject.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment {
                                                register: memory.register.clone(),
                                                expression: lambda.clone().into()
                                            }.into(),
                                            IntermediateAssignment {
                                                register: target.register.clone(),
                                                expression: IntermediateValue::from(
                                                    Boolean{value: false}
                                                ).into()
                                            }.into(),
                                        ],
                                        target.clone().into()
                                    ).into()
                                },
                            ],
                        }.into()
                    }.into(),
                ]
            )
        },
        true;
        "match statement"
    )]
    fn test_inlining(
        statements_expected: (Vec<IntermediateStatement>, Vec<IntermediateStatement>),
        expect_continue: bool,
    ) {
        let (statements, expected) = statements_expected;
        let lambda = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                ret: Integer { value: 0 }.into(),
                statements,
            },
        };
        let (optimized, should_continue) = Inliner::inline_iteration(lambda, None);
        assert_eq!(expect_continue, should_continue);

        let expected = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                ret: Integer { value: 0 }.into(),
                statements: expected,
            },
        };
        dbg!(&expected, &optimized);
        ExpressionEqualityChecker::assert_equal(&optimized.into(), &expected.into())
    }

    #[test]
    fn test_main_inlining() {
        let premain = IntermediateMemory {
            register: Register::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let call = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let simplified = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                ret: Integer { value: 0 }.into(),
                statements: Vec::new(),
            },
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        expression: simplified.clone().into(),
                        register: premain.register.clone(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: call.register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: Vec::new(),
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: call.clone().into(),
            },
        };
        let types = vec![Rc::new(RefCell::new(
            IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()), None]).into(),
        ))];
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main,
                types: types.clone(),
            },
            None,
        );
        dbg!(&simplified, &optimized.main);
        ExpressionEqualityChecker::assert_equal(&optimized.main.into(), &simplified.into());
        assert_eq!(types, optimized.types)
    }

    #[test]
    fn test_size_limited_inlining() {
        let premain = IntermediateMemory {
            register: Register::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let call = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let simplified = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                ret: Integer { value: 0 }.into(),
                statements: Vec::new(),
            },
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        expression: simplified.clone().into(),
                        register: premain.register.clone(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: call.register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: Vec::new(),
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: call.clone().into(),
            },
        };
        let types = vec![Rc::new(RefCell::new(
            IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()), None]).into(),
        ))];
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: types.clone(),
            },
            Some(1),
        );
        dbg!(&main, &optimized.main);
        ExpressionEqualityChecker::assert_equal(&optimized.main.into(), &main.into());
        assert_eq!(types, optimized.types)
    }

    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateLambda {
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment{
                            expression: IntermediateLambda{
                                args: Vec::new(),
                                block: IntermediateBlock {
                                    statements: vec![
                                        IntermediateAssignment{
                                            register: foo_call.register.clone(),
                                            expression: IntermediateFnCall{
                                                fn_: foo.clone().into(),
                                                args: Vec::new()
                                            }.into()
                                        }.into()
                                    ],
                                    ret: foo_call.clone().into()
                                },
                            }.into(),
                            register: foo.register.clone()
                        }.into(),
                        IntermediateAssignment{
                            register: main_call.register.clone(),
                            expression: IntermediateFnCall{
                                fn_: foo.clone().into(),
                                args: Vec::new()
                            }.into()
                        }.into()
                    ],
                    ret: main_call.clone().into(),
                },
                args: Vec::new(),
            }
        };
        "self recursive fn"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateLambda {
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment{
                        expression: IntermediateLambda{
                                args: Vec::new(),
                                block: IntermediateBlock {
                                    statements: vec![
                                        IntermediateAssignment{
                                            register: bar_call.register.clone(),
                                            expression: IntermediateFnCall{
                                                fn_: bar.clone().into(),
                                                args: Vec::new()
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
                                args: Vec::new(),
                                block: IntermediateBlock {
                                    statements: vec![
                                        IntermediateAssignment{
                                            register: foo_call.register.clone(),
                                            expression: IntermediateFnCall{
                                                fn_: foo.clone().into(),
                                                args: Vec::new()
                                            }.into()
                                        }.into()
                                    ],
                                    ret: foo_call.clone().into()
                                },
                            }.into(),
                            register: bar.register.clone()
                        }.into(),
                        IntermediateAssignment{
                            register: main_call.register.clone(),
                            expression: IntermediateFnCall{
                                fn_: foo.clone().into(),
                                args: Vec::new()
                            }.into()
                        }.into()
                    ],
                    ret: main_call.clone().into(),
                },
                args: Vec::new(),
            }
        };
        "mutually recursive fns"
    )]
    fn test_iterative_inlining(lambda: IntermediateLambda) {
        let mut program = IntermediateProgram {
            main: lambda,
            types: Vec::new(),
        };
        for _ in 1..5 {
            let size = CodeSizeEstimator::estimate_size(&program.main);
            program = Inliner::inline_up_to_size(program, Some(size.1));
            assert!(program.main.find_open_vars().is_empty());
        }
    }

    #[test]
    fn test_recursive_inlining() {
        let premain = IntermediateMemory {
            register: Register::new(),
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
        };
        let call = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };

        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let ret = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let calls = [
            IntermediateMemory {
                register: Register::new(),
                type_: AtomicTypeEnum::INT.into(),
            },
            IntermediateMemory {
                register: Register::new(),
                type_: AtomicTypeEnum::INT.into(),
            },
        ];
        let recursive = IntermediateLambda {
            args: vec![arg.clone()],
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        register: calls[0].register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: vec![arg.clone().into()],
                        }
                        .into(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: calls[1].register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: vec![arg.clone().into()],
                        }
                        .into(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: ret.register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: BuiltInFn(
                                Id::from("+"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into()),
                                )
                                .into(),
                            )
                            .into(),
                            args: vec![calls[0].clone().into(), calls[1].clone().into()],
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: ret.clone().into(),
            },
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        expression: recursive.clone().into(),
                        register: premain.register.clone(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: call.register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: vec![Integer { value: 10 }.into()],
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: call.clone().into(),
            },
        };
        let current_size = CodeSizeEstimator::estimate_size(&recursive).1;
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main,
                types: Vec::new(),
            },
            Some(current_size * 10),
        );
        dbg!(&optimized);
        let optimized_size = CodeSizeEstimator::estimate_size(&optimized.main).1;
        assert!(optimized_size > current_size * 2);
        assert!(optimized_size < current_size * 40);
    }

    #[test]
    fn test_self_recursive_inlining() {
        let premain = IntermediateMemory {
            register: Register::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let call = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };

        let ret = IntermediateMemory {
            register: Register::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let recursive = IntermediateLambda {
            args: vec![arg.clone()],
            block: IntermediateBlock {
                statements: vec![IntermediateAssignment {
                    register: ret.register.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: vec![arg.clone().into()],
                    }
                    .into(),
                }
                .into()],
                ret: ret.clone().into(),
            },
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: vec![
                    IntermediateAssignment {
                        expression: recursive.clone().into(),
                        register: premain.register.clone(),
                    }
                    .into(),
                    IntermediateAssignment {
                        register: call.register.clone().into(),
                        expression: IntermediateFnCall {
                            fn_: premain.clone().into(),
                            args: vec![Integer { value: -10 }.into()],
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: call.clone().into(),
            },
        };
        let current_size = CodeSizeEstimator::estimate_size(&recursive).1;
        Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: Vec::new(),
            },
            Some(current_size * 10),
        );

        Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: Vec::new(),
            },
            None,
        );
    }
}
