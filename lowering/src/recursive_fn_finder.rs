use std::collections::HashMap;

use crate::{
    fn_inst::{FnDefs, FnInst},
    IntermediateAssignment, IntermediateExpression, IntermediateFnCall, IntermediateLambda,
    IntermediateMemory, IntermediateProgram, IntermediateStatement, IntermediateValue,
};
use itertools::Either::{Left, Right};

pub type RecursiveFns = HashMap<IntermediateLambda, bool>;

pub struct RecursiveFnFinder {
    fn_defs: FnDefs,
}

impl RecursiveFnFinder {
    /// Find whether all functions in a program are recursive or not.
    pub fn recursive_fns(program: &IntermediateProgram) -> RecursiveFns {
        let lambda = &program.main;
        let mut finder = RecursiveFnFinder {
            fn_defs: FnDefs::new(),
        };
        FnInst::collect_fn_defs_from_statements(&lambda.block.statements, &mut finder.fn_defs);
        let mut recursive_fns = RecursiveFns::new();
        for value in finder.fn_defs.values() {
            if let FnInst::Lambda(lambda) = value {
                finder.find(&lambda, &mut recursive_fns);
            }
        }
        recursive_fns
    }
    /// Find out whether a lambda is recursive.
    fn find(&self, lambda: &IntermediateLambda, recursive_fns: &mut RecursiveFns) -> bool {
        if recursive_fns.contains_key(lambda) {
            return recursive_fns[lambda];
        }
        // Assume that it is recursive, in case it is found.
        recursive_fns.insert(lambda.clone(), true);
        let is_recursive = self.check(&lambda.block.statements, recursive_fns);
        // Update based on result.
        recursive_fns.insert(lambda.clone(), is_recursive);
        is_recursive
    }
    /// Check for calls to recursive functions within the body.
    fn check(
        &self,
        statements: &Vec<IntermediateStatement>,
        recursive_fns: &mut HashMap<IntermediateLambda, bool>,
    ) -> bool {
        statements.iter().any(|statement| match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                register: _,
            }) => match expression {
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args: _ }) => {
                    match fn_ {
                        IntermediateValue::IntermediateBuiltIn(_) => false,
                        IntermediateValue::IntermediateMemory(IntermediateMemory {
                            type_: _,
                            register,
                        }) => match FnInst::get_root_fn(&self.fn_defs, register) {
                            Some(Left(lambda)) => self.find(&lambda, recursive_fns),
                            Some(Right(_)) => false,
                            None => true,
                        },
                        IntermediateValue::IntermediateArg(_) => true,
                    }
                }
                IntermediateExpression::IntermediateIf(if_) => {
                    self.check(&if_.branches.0.statements, recursive_fns)
                        || self.check(&if_.branches.1.statements, recursive_fns)
                }
                IntermediateExpression::IntermediateMatch(match_) => match_
                    .branches
                    .iter()
                    .any(|branch| self.check(&branch.block.statements, recursive_fns)),
                _ => false,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        IntermediateArg, IntermediateAssignment, IntermediateBlock, IntermediateFnCall,
        IntermediateFnType, IntermediateIf, IntermediateLambda, IntermediateMemory,
        IntermediateStatement, IntermediateType, Register,
    };

    use super::*;

    use test_case::test_case;
    use type_checker::{AtomicTypeEnum, Boolean, Integer};

    #[test_case(
        {
            (
                Vec::new(),
                HashMap::new()
            )
        };
        "empty fns"
    )]
    #[test_case(
        {
            let identity = Register::new();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            (
                vec![
                    IntermediateAssignment{
                        register: identity.clone(),
                        expression: IntermediateLambda {
                            args: vec![arg.clone()],
                            block: IntermediateBlock {
                                statements: Vec::new(),
                                ret: arg.clone().into()
                            },
                        }.into()
                    }.into(),
                ],
                HashMap::from([
                    (identity, false)
                ])
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let call = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            let fn_ = Register::new();
            (
                vec![
                    IntermediateAssignment{
                        register: fn_.clone(),
                        expression: IntermediateLambda {
                            block: IntermediateBlock{
                                statements: vec![
                                    IntermediateAssignment{
                                        register: call.register.clone(),
                                        expression: IntermediateFnCall{
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
                                    }.into()
                                ],
                                ret: call.clone().into()
                            },
                            args: Vec::new()
                        }.into()
                    }.into()
                ],
                HashMap::from([
                    (fn_, true)
                ])
            )
        };
        "higher order fn"
    )]
    #[test_case(
        {
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
            let idea_fn = IntermediateLambda {
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
            let idea = Register::new();
            (
                vec![
                    IntermediateAssignment{
                        register: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), false),
                    (id_fn.register.clone(), false)
                ])
            )
        };
        "internal non-recursive fn call"
    )]
    #[test_case(
        {
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
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
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
            let idea = Register::new();
            (
                vec![
                    IntermediateAssignment {
                        register: id_fn.register.clone(),
                        expression: id.clone().into(),
                    }
                    .into(),
                    IntermediateAssignment{
                        register: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), false),
                    (id_fn.register.clone(), false)
                ])
            )
        };
        "external non-recursive fn call"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
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
                ],
                HashMap::from([
                    (foo.register.clone(), true)
                ])
            )
        };
        "self-recursive fn"
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
            (
                vec![
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
                ],
                HashMap::from([
                    (foo.register.clone(), true),
                    (bar.register.clone(), true)
                ])
            )
        };
        "mutually recursive fns"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            register: idea_ret.register.clone(),
                            expression: IntermediateFnCall {
                                fn_: foo.clone().into(),
                                args: Vec::new(),
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Register::new();
            (
                vec![
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
                        register: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), true),
                    (foo.register.clone(), true)
                ])
            )
        };
        "recursive fn call"
    )]
    #[test_case(
        {
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

            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let call_a = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let call_b = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            register: idea_ret.register.clone(),
                            expression: IntermediateIf {
                                condition: Boolean{value: true}.into(),
                                branches: (
                                    IntermediateBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                register: call_a.register.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: foo.clone().into(),
                                                    args: Vec::new()
                                                }.into()
                                            }.into()
                                        ],
                                        ret: call_a.clone().into()
                                    },
                                    IntermediateBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                register: call_b.register.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: id_fn.clone().into(),
                                                    args: vec![
                                                        idea_arg.clone().into()
                                                    ]
                                                }.into()
                                            }.into()
                                        ],
                                        ret: call_b.clone().into()
                                    },
                                )
                            }.into()
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Register::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: id.clone().into(),
                        register: id_fn.register.clone(),
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
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        register: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), true),
                    (id_fn.register.clone(),false),
                    (foo.register.clone(), true)
                ])
            )
        };
        "mixed fn call"
    )]
    fn test_recursive_fn_finder(fns: (Vec<IntermediateStatement>, HashMap<Register, bool>)) {
        let (statements, recursive) = fns;
        let mut fn_defs = HashMap::new();
        FnInst::collect_fn_defs_from_statements(&statements, &mut fn_defs);
        let expected = recursive
            .into_iter()
            .map(
                |(register, recursive)| match FnInst::get_root_fn(&fn_defs, &register) {
                    Some(Left(lambda)) => (lambda, recursive),
                    _ => panic!(),
                },
            )
            .collect();
        let recursive_fns = RecursiveFnFinder::recursive_fns(&IntermediateProgram {
            main: IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements,
                    ret: Integer { value: 0 }.into(),
                },
            },
            types: Vec::new(),
        });
        assert_eq!(recursive_fns, expected);
    }
}
