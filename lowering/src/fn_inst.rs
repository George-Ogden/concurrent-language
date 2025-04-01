use std::collections::HashMap;

use itertools::Either::{self, Left, Right};

use crate::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateLambda, IntermediateStatement, IntermediateValue, Register,
};

#[derive(Debug, PartialEq, Clone)]
/// FnInst stores all ways of identifying a function.
pub enum FnInst {
    Lambda(IntermediateLambda),
    BuiltIn(BuiltInFn),
    Ref(Register),
}

impl From<IntermediateLambda> for FnInst {
    fn from(value: IntermediateLambda) -> Self {
        FnInst::Lambda(value)
    }
}

impl From<BuiltInFn> for FnInst {
    fn from(value: BuiltInFn) -> Self {
        FnInst::BuiltIn(value)
    }
}

impl From<Register> for FnInst {
    fn from(value: Register) -> Self {
        FnInst::Ref(value)
    }
}

pub type FnDefs = HashMap<Register, FnInst>;

impl FnInst {
    fn collect_fn_defs_from_statement(statement: &IntermediateStatement, fn_defs: &mut FnDefs) {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::collect_fns_defs_from_assignment(assignment, fn_defs)
            }
        }
    }
    fn collect_fns_defs_from_assignment(
        IntermediateAssignment {
            expression,
            register,
        }: &IntermediateAssignment,
        fn_defs: &mut FnDefs,
    ) {
        match expression {
            IntermediateExpression::IntermediateLambda(lambda) => {
                fn_defs.insert(register.clone(), lambda.clone().into());
                Self::collect_fn_defs_from_statements(&lambda.block.statements, fn_defs);
            }
            IntermediateExpression::IntermediateValue(IntermediateValue::IntermediateBuiltIn(
                IntermediateBuiltIn::BuiltInFn(fn_),
            )) => {
                fn_defs.insert(register.clone(), fn_.clone().into());
            }
            IntermediateExpression::IntermediateValue(IntermediateValue::IntermediateMemory(
                memory,
            )) if fn_defs.contains_key(&memory.register) => {
                fn_defs.insert(register.clone(), memory.register.clone().into());
            }
            IntermediateExpression::IntermediateIf(if_) => {
                Self::collect_fn_defs_from_statements(&if_.branches.0.statements, fn_defs);
                Self::collect_fn_defs_from_statements(&if_.branches.1.statements, fn_defs);
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                for branch in &match_.branches {
                    Self::collect_fn_defs_from_statements(&branch.block.statements, fn_defs);
                }
            }
            _ => {}
        }
    }
    pub fn collect_fn_defs_from_statements(
        statements: &Vec<IntermediateStatement>,
        fn_defs: &mut FnDefs,
    ) {
        for statement in statements {
            Self::collect_fn_defs_from_statement(statement, fn_defs);
        }
    }

    /// Translate a register into a lambda or built-in function, if it can be traced.
    pub fn get_root_fn(
        fn_defs: &FnDefs,
        register: &Register,
    ) -> Option<Either<IntermediateLambda, BuiltInFn>> {
        let fn_def = fn_defs.get(&register);
        match fn_def {
            Some(FnInst::Lambda(lambda)) => Some(Left(lambda.clone())),
            Some(FnInst::BuiltIn(built_in_fn)) => Some(Right(built_in_fn.clone())),
            Some(FnInst::Ref(register)) => Self::get_root_fn(fn_defs, register),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        IntermediateArg, IntermediateBlock, IntermediateBuiltIn, IntermediateFnCall,
        IntermediateFnType, IntermediateIf, IntermediateMatch, IntermediateMatchBranch,
        IntermediateMemory, IntermediateType, IntermediateUnionType,
    };

    use super::*;

    use test_case::test_case;
    use type_checker::{AtomicTypeEnum, Id, Integer};

    #[test_case(
        {
            (
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateFnCall {
                            fn_: IntermediateBuiltIn::from(BuiltInFn(
                                Id::from("++"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into()),
                                )
                            )).into(),
                            args: vec![IntermediateMemory{
                                register: Register::new(),
                                type_: AtomicTypeEnum::INT.into()
                            }.into()]
                        }.into()
                    }.into()
                ],
                FnDefs::new()
            )
        };
        "no lambda defs"
    )]
    #[test_case(
        {
            let register = Register::new();
            let lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            (
                vec![
                    IntermediateAssignment {
                        register: register.clone(),
                        expression: lambda.clone().into()
                    }.into()
                ],
                FnDefs::from([
                    (register, lambda.into())
                ])
            )
        };
        "single lambda def"
    )]
    #[test_case(
        {
            let register_a = Register::new();
            let register_b = Register::new();
            let lambda_a = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            let lambda_b = IntermediateLambda {
                args: vec![arg.clone()],
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: arg.clone().into()
                },
            };
            (
                vec![
                    IntermediateAssignment {
                        register: register_a.clone(),
                        expression: lambda_a.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: register_b.clone(),
                        expression: lambda_b.clone().into()
                    }.into(),
                ],
                FnDefs::from([
                    (register_a, lambda_a.into()),
                    (register_b, lambda_b.into()),
                ])
            )
        };
        "multiple lambda defs"
    )]
    #[test_case(
        {
            let register = Register::new();
            let fn_ = BuiltInFn(
                Id::from("<=>"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            );
            (
                vec![
                    IntermediateAssignment {
                        register: register.clone(),
                        expression: IntermediateValue::from(fn_.clone()).into()
                    }.into()
                ],
                FnDefs::from([
                    (register, fn_.into())
                ])
            )
        };
        "built-in fn assignment"
    )]
    #[test_case(
        {
            let memory = IntermediateMemory{
                register: Register::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let register = Register::new();
            let lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            (
                vec![
                    IntermediateAssignment {
                        register: memory.register.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        register: register.clone(),
                        expression: IntermediateValue::from(memory.clone()).into()
                    }.into()
                ],
                FnDefs::from([
                    (memory.register.clone(), lambda.into()),
                    (register, memory.register.into()),
                ])
            )
        };
        "reassignment"
    )]
    #[test_case(
        {
            let lambda_0 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let assignment_0 = IntermediateAssignment {
                expression: lambda_0.clone().into(),
                register: Register::new()
            };
            let lambda_1 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 13}.into()
                },
            };
            let assignment_1 = IntermediateAssignment {
                expression: lambda_1.clone().into(),
                register: Register::new()
            };
            (
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateIf {
                            condition: IntermediateArg{
                                register: Register::new(),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            branches: (
                                (
                                    vec![
                                        assignment_0.clone().into()
                                    ],
                                    IntermediateValue::from(assignment_0.clone()).into()
                                ).into(),
                                (
                                    vec![
                                        assignment_1.clone().into()
                                    ],
                                    IntermediateValue::from(assignment_1.clone()).into()
                                ).into(),
                            )
                        }.into(),
                    }.into()
                ],
                FnDefs::from([
                    (assignment_0.register, lambda_0.into()),
                    (assignment_1.register, lambda_1.into()),
                ])
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let lambda_0 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let lambda_1 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 13}.into()
                },
            };
            let lambda_2 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 13}.into()
                },
            };
            let assignment_0 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_0.clone().into(),
            };
            let assignment_1 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_1.clone().into(),
            };
            let assignment_2 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_2.clone().into(),
            };
            (
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                register: Register::new(),
                                type_: IntermediateUnionType(vec![None,None,None]).into()
                            }.into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![assignment_0.clone().into()],
                                        IntermediateValue::from(assignment_0.clone()).clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![assignment_1.clone().into()],
                                        IntermediateValue::from(assignment_1.clone()).clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![assignment_2.clone().into()],
                                        IntermediateValue::from(assignment_2.clone()).clone().into()
                                    ).into()
                                },
                            ]
                        }.into(),
                    }.into()
                ],
                FnDefs::from([
                    (assignment_0.register, lambda_0.into()),
                    (assignment_1.register, lambda_1.into()),
                    (assignment_2.register, lambda_2.into()),
                ])
            )
        };
        "match statement"
    )]
    #[test_case(
        {
            let lambda_0 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let lambda_1 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 13}.into()
                },
            };
            let lambda_2 = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 13}.into()
                },
            };
            let assignment_0 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_0.clone().into(),
            };
            let assignment_1 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_1.clone().into(),
            };
            let assignment_2 = IntermediateAssignment {
                register: Register::new(),
                expression: lambda_2.clone().into(),
            };
            (
                vec![
                    assignment_0.clone().into(),
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                register: Register::new(),
                                type_: IntermediateUnionType(vec![None,None,None]).into()
                            }.into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![assignment_1.clone().into()],
                                        IntermediateValue::from(assignment_1.clone()).clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch {
                                    target: None,
                                    block: (
                                        vec![assignment_2.clone().into()],
                                        IntermediateValue::from(assignment_2.clone()).clone().into()
                                    ).into()
                                },
                            ]
                        }.into(),
                    }.into()
                ],
                FnDefs::from([
                    (assignment_0.register, lambda_0.into()),
                    (assignment_1.register, lambda_1.into()),
                    (assignment_2.register, lambda_2.into()),
                ])
            )
        };
        "match statement with pre-definition"
    )]
    #[test_case(
        {
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: arg.clone().into()
                },
            };
            let assignment = IntermediateAssignment {
                register: Register::new(),
                expression: lambda.clone().into(),
            };
            (
                vec![
                    IntermediateAssignment {
                        register: Register::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                register: Register::new(),
                                type_: IntermediateUnionType(vec![None,None,None]).into()
                            }.into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: Some(arg.clone()),
                                    block: (
                                        vec![assignment.clone().into()],
                                        IntermediateValue::from(assignment.clone()).clone().into()
                                    ).into()
                                },
                            ]
                        }.into(),
                    }.into()
                ],
                FnDefs::from([
                    (assignment.register, lambda.into()),
                ])
            )
        };
        "match statement single branch"
    )]
    #[test_case(
        {
            let internal_register = Register::new();
            let external_register = Register::new();
            let ret_reg = Register::new();
            let internal_lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let external_lambda = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            register: internal_register.clone(),
                            expression: internal_lambda.clone().into()
                        }.into(),
                        IntermediateAssignment {
                            register: ret_reg.clone(),
                            expression: IntermediateFnCall{
                                fn_: IntermediateMemory {
                                    register: internal_register.clone(),
                                    type_: IntermediateFnType(
                                        Vec::new(),
                                        Box::new(AtomicTypeEnum::INT.into())
                                    ).into()
                                }.into(),
                                args: Vec::new()
                            }.into()
                        }.into(),
                    ],
                    ret: IntermediateMemory {
                        register: ret_reg,
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                }
            };
            (
                vec![
                    IntermediateAssignment {
                        register: external_register.clone(),
                        expression: external_lambda.clone().into()
                    }.into(),
                ],
                FnDefs::from([
                    (internal_register, internal_lambda.into()),
                    (external_register, external_lambda.into()),
                ])
            )
        };
        "nested lambda defs"
    )]
    fn test_collect_fn_defs(statements_fns: (Vec<IntermediateStatement>, FnDefs)) {
        let (statements, expected_fn_defs) = statements_fns;
        let mut fn_defs = FnDefs::new();
        FnInst::collect_fn_defs_from_statements(&statements, &mut fn_defs);
        assert_eq!(fn_defs, expected_fn_defs)
    }
}
