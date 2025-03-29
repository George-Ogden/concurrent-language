use std::collections::HashMap;

use itertools::Either::{self, Left, Right};

use crate::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateLambda, IntermediateStatement, IntermediateValue, Location,
};

#[derive(Debug, PartialEq, Clone)]
/// FnInst stores all ways of identifying a function.
pub enum FnInst {
    Lambda(IntermediateLambda),
    BuiltIn(BuiltInFn),
    Ref(Location),
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

impl From<Location> for FnInst {
    fn from(value: Location) -> Self {
        FnInst::Ref(value)
    }
}

pub type FnDefs = HashMap<Location, FnInst>;

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
            location,
        }: &IntermediateAssignment,
        fn_defs: &mut FnDefs,
    ) {
        match expression {
            IntermediateExpression::IntermediateLambda(lambda) => {
                fn_defs.insert(location.clone(), lambda.clone().into());
                Self::collect_fn_defs_from_statements(&lambda.block.statements, fn_defs);
            }
            IntermediateExpression::IntermediateValue(IntermediateValue::IntermediateBuiltIn(
                IntermediateBuiltIn::BuiltInFn(fn_),
            )) => {
                fn_defs.insert(location.clone(), fn_.clone().into());
            }
            IntermediateExpression::IntermediateValue(IntermediateValue::IntermediateMemory(
                memory,
            )) if fn_defs.contains_key(&memory.location) => {
                fn_defs.insert(location.clone(), memory.location.clone().into());
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

    /// Translate a location into a lambda or built-in function, if it can be traced.
    pub fn get_root_fn(
        fn_defs: &FnDefs,
        location: &Location,
    ) -> Option<Either<IntermediateLambda, BuiltInFn>> {
        let fn_def = fn_defs.get(&location);
        match fn_def {
            Some(FnInst::Lambda(lambda)) => Some(Left(lambda.clone())),
            Some(FnInst::BuiltIn(built_in_fn)) => Some(Right(built_in_fn.clone())),
            Some(FnInst::Ref(location)) => Self::get_root_fn(fn_defs, location),
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
                        location: Location::new(),
                        expression: IntermediateFnCall {
                            fn_: IntermediateBuiltIn::from(BuiltInFn(
                                Id::from("++"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into()),
                                )
                            )).into(),
                            args: vec![IntermediateMemory{
                                location: Location::new(),
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
            let location = Location::new();
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
                        location: location.clone(),
                        expression: lambda.clone().into()
                    }.into()
                ],
                FnDefs::from([
                    (location, lambda.into())
                ])
            )
        };
        "single lambda def"
    )]
    #[test_case(
        {
            let location_a = Location::new();
            let location_b = Location::new();
            let lambda_a = IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock{
                    statements: Vec::new(),
                    ret: Integer{value: 11}.into()
                },
            };
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
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
                        location: location_a.clone(),
                        expression: lambda_a.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: location_b.clone(),
                        expression: lambda_b.clone().into()
                    }.into(),
                ],
                FnDefs::from([
                    (location_a, lambda_a.into()),
                    (location_b, lambda_b.into()),
                ])
            )
        };
        "multiple lambda defs"
    )]
    #[test_case(
        {
            let location = Location::new();
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
                        location: location.clone(),
                        expression: IntermediateValue::from(fn_.clone()).into()
                    }.into()
                ],
                FnDefs::from([
                    (location, fn_.into())
                ])
            )
        };
        "built-in fn assignment"
    )]
    #[test_case(
        {
            let memory = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let location = Location::new();
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
                        location: memory.location.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: location.clone(),
                        expression: IntermediateValue::from(memory.clone()).into()
                    }.into()
                ],
                FnDefs::from([
                    (memory.location.clone(), lambda.into()),
                    (location, memory.location.into()),
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
                location: Location::new()
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
                location: Location::new()
            };
            (
                vec![
                    IntermediateAssignment {
                        location: Location::new(),
                        expression: IntermediateIf {
                            condition: IntermediateArg{
                                location: Location::new(),
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
                    (assignment_0.location, lambda_0.into()),
                    (assignment_1.location, lambda_1.into()),
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
                location: Location::new(),
                expression: lambda_0.clone().into(),
            };
            let assignment_1 = IntermediateAssignment {
                location: Location::new(),
                expression: lambda_1.clone().into(),
            };
            let assignment_2 = IntermediateAssignment {
                location: Location::new(),
                expression: lambda_2.clone().into(),
            };
            (
                vec![
                    IntermediateAssignment {
                        location: Location::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                location: Location::new(),
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
                    (assignment_0.location, lambda_0.into()),
                    (assignment_1.location, lambda_1.into()),
                    (assignment_2.location, lambda_2.into()),
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
                location: Location::new(),
                expression: lambda_0.clone().into(),
            };
            let assignment_1 = IntermediateAssignment {
                location: Location::new(),
                expression: lambda_1.clone().into(),
            };
            let assignment_2 = IntermediateAssignment {
                location: Location::new(),
                expression: lambda_2.clone().into(),
            };
            (
                vec![
                    assignment_0.clone().into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                location: Location::new(),
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
                    (assignment_0.location, lambda_0.into()),
                    (assignment_1.location, lambda_1.into()),
                    (assignment_2.location, lambda_2.into()),
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
                location: Location::new(),
                expression: lambda.clone().into(),
            };
            (
                vec![
                    IntermediateAssignment {
                        location: Location::new(),
                        expression: IntermediateMatch {
                            subject: IntermediateArg{
                                location: Location::new(),
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
                    (assignment.location, lambda.into()),
                ])
            )
        };
        "match statement single branch"
    )]
    #[test_case(
        {
            let internal_location = Location::new();
            let external_location = Location::new();
            let ret_loc = Location::new();
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
                            location: internal_location.clone(),
                            expression: internal_lambda.clone().into()
                        }.into(),
                        IntermediateAssignment {
                            location: ret_loc.clone(),
                            expression: IntermediateFnCall{
                                fn_: IntermediateMemory {
                                    location: internal_location.clone(),
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
                        location: ret_loc,
                        type_: AtomicTypeEnum::INT.into()
                    }.into(),
                }
            };
            (
                vec![
                    IntermediateAssignment {
                        location: external_location.clone(),
                        expression: external_lambda.clone().into()
                    }.into(),
                ],
                FnDefs::from([
                    (internal_location, internal_lambda.into()),
                    (external_location, external_lambda.into()),
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
