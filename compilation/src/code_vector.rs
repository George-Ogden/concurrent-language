use std::collections::HashMap;

use itertools::Itertools;
use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateFnCall, IntermediateIfStatement, IntermediateLambda, IntermediateMatchBranch,
    IntermediateMatchStatement, IntermediateStatement, IntermediateValue,
};

use crate::code_size::{CodeVector, CODE_SIZE_CONSTANTS};

pub struct CodeVectorCalculator {}

impl CodeVectorCalculator {
    fn builtin_vector(built_in: &IntermediateBuiltIn) -> CodeVector {
        match built_in {
            IntermediateBuiltIn::Integer(_) => CodeVector::builtin_int(),
            IntermediateBuiltIn::Boolean(_) => CodeVector::builtin_bool(),
            IntermediateBuiltIn::BuiltInFn(_) => CodeVector::builtin_fn(),
        }
    }
    fn value_vector(value: &IntermediateValue) -> CodeVector {
        match value {
            IntermediateValue::IntermediateBuiltIn(built_in) => Self::builtin_vector(built_in),
            IntermediateValue::IntermediateMemory(_) | IntermediateValue::IntermediateArg(_) => {
                CodeVector::memory_access()
            }
        }
    }
    fn values_vector(values: &Vec<IntermediateValue>) -> CodeVector {
        values.iter().map(Self::value_vector).sum()
    }

    fn expression_vector(expression: &IntermediateExpression) -> CodeVector {
        let values_vector = Self::values_vector(&expression.values());
        match expression {
            IntermediateExpression::IntermediateValue(_) => {
                CodeVector::value_expression() + values_vector
            }
            IntermediateExpression::IntermediateElementAccess(_) => {
                CodeVector::element_access() + values_vector
            }
            IntermediateExpression::IntermediateTupleExpression(_) => {
                CodeVector::tuple_expression() + values_vector
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                fn_:
                    IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::BuiltInFn(BuiltInFn(
                        id,
                        _,
                    ))),
                args,
            }) => CodeVector::operator(id.clone()) + Self::values_vector(args),
            IntermediateExpression::IntermediateFnCall(_) => CodeVector::fn_call() + values_vector,
            IntermediateExpression::IntermediateCtorCall(_) => {
                CodeVector::ctor_call() + values_vector
            }
            IntermediateExpression::IntermediateLambda(_) => CodeVector::lambda(),
        }
    }

    fn statement_vector(statement: &IntermediateStatement) -> CodeVector {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::assignment_vector(assignment)
            }
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                Self::if_statement_vector(if_statement)
            }
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                Self::match_statement_vector(match_statement)
            }
        }
    }
    fn assignment_vector(assignment: &IntermediateAssignment) -> CodeVector {
        Self::expression_vector(&assignment.expression) + CodeVector::assignment()
    }
    fn if_statement_vector(if_statement: &IntermediateIfStatement) -> CodeVector {
        let condition_vector = Self::value_vector(&if_statement.condition);
        let branch_vectors = (
            Self::statements_vector(&if_statement.branches.0),
            Self::statements_vector(&if_statement.branches.1),
        );
        if &branch_vectors.0 != &branch_vectors.1 {
            panic!("If statement branches are not equal")
        }
        branch_vectors.0 + condition_vector + CodeVector::if_statement()
    }
    fn match_statement_vector(match_statement: &IntermediateMatchStatement) -> CodeVector {
        let subject_vector = Self::value_vector(&match_statement.subject);
        let branch_vectors = match_statement
            .branches
            .iter()
            .map(Self::match_branch_vector)
            .collect_vec();
        if !branch_vectors.iter().all_equal() {
            panic!("Match statement branches are not equal")
        }
        branch_vectors[0].clone() + subject_vector + CodeVector::match_statement()
    }
    fn match_branch_vector(match_branch: &IntermediateMatchBranch) -> CodeVector {
        Self::statements_vector(&match_branch.statements)
    }
    fn statements_vector(statements: &Vec<IntermediateStatement>) -> CodeVector {
        statements.iter().map(Self::statement_vector).sum()
    }
    fn lambda_vector(lambda: &IntermediateLambda) -> CodeVector {
        let mut default = CodeVector::new();
        default.operators =
            HashMap::from_iter(CODE_SIZE_CONSTANTS.operators.keys().map(|k| (k.clone(), 0)));
        Self::statements_vector(&lambda.statements) + Self::value_vector(&lambda.ret) + default
    }
}

#[cfg(test)]
mod tests {

    use std::panic::{self, AssertUnwindSafe};

    use super::*;

    use itertools::Itertools;
    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, Id, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateCtorCall, IntermediateElementAccess, IntermediateFnCall, IntermediateFnType,
        IntermediateIfStatement, IntermediateLambda, IntermediateMatchBranch,
        IntermediateMatchStatement, IntermediateMemory, IntermediateStatement,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateUnionType, Location,
        DEFAULT_CONTEXT,
    };
    use once_cell::sync::Lazy;
    use test_case::test_case;

    const BBV: Lazy<CodeVector> = Lazy::new(|| CodeVector::builtin_bool());
    const BIV: Lazy<CodeVector> = Lazy::new(|| CodeVector::builtin_int());
    const BFV: Lazy<CodeVector> = Lazy::new(|| CodeVector::builtin_fn());
    const MAV: Lazy<CodeVector> = Lazy::new(|| CodeVector::memory_access());
    const TEV: Lazy<CodeVector> = Lazy::new(|| CodeVector::tuple_expression());
    const EAV: Lazy<CodeVector> = Lazy::new(|| CodeVector::element_access());
    const VEV: Lazy<CodeVector> = Lazy::new(|| CodeVector::value_expression());
    const FCV: Lazy<CodeVector> = Lazy::new(|| CodeVector::fn_call());
    const CCV: Lazy<CodeVector> = Lazy::new(|| CodeVector::ctor_call());
    const LV: Lazy<CodeVector> = Lazy::new(|| CodeVector::lambda());
    const AV: Lazy<CodeVector> = Lazy::new(|| CodeVector::assignment());
    const ISV: Lazy<CodeVector> = Lazy::new(|| CodeVector::if_statement());
    const MSV: Lazy<CodeVector> = Lazy::new(|| CodeVector::match_statement());

    #[test_case(
        IntermediateValue::from(Boolean{value: true}),
        BBV.clone();
        "bool"
    )]
    #[test_case(
        IntermediateValue::from(Integer{value: 5}),
        BIV.clone();
        "int"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
            Id::from("+"),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            )
        ))),
        BFV.clone();
        "built-in function"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateArg{
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new()
        }),
        MAV.clone();
        "argument"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateMemory{
            type_: AtomicTypeEnum::BOOL.into(),
            location: Location::new()
        }),
        MAV.clone();
        "memory"
    )]
    fn test_value_vector(value: IntermediateValue, expected_vector: CodeVector) {
        let vector = CodeVectorCalculator::value_vector(&value);
        assert_eq!(vector, expected_vector)
    }

    #[test_case(
        IntermediateTupleExpression(Vec::new()).into(),
        TEV.clone();
        "empty tuple"
    )]
    #[test_case(
        IntermediateTupleExpression(vec![
            IntermediateMemory{type_: AtomicTypeEnum::BOOL.into(), location: Location::new()}.into(),
            Integer{value: 43}.into()
        ]).into(),
        TEV.clone() + BIV.clone() + MAV.clone();
        "non-empty tuple"
    )]
    #[test_case(
        IntermediateElementAccess{
            idx: 1,
            value: IntermediateMemory{
                location: Location::new(),
                type_: IntermediateTupleType(vec![
                    AtomicTypeEnum::BOOL.into(),
                    AtomicTypeEnum::INT.into(),
                ]).into()
            }.into()
        }.into(),
        EAV.clone() + MAV.clone();
        "tuple access"
    )]
    #[test_case(
        IntermediateValue::from(Integer{value: -27}).into(),
        VEV.clone() + BIV.clone();
        "value expression"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateMemory{
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into(),
                location: Location::new()
            }.into(),
            args: Vec::new(),
        }.into(),
        FCV.clone() + MAV.clone();
        "user-defined fn-call no args"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateMemory{
                type_: IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into(),
                location: Location::new()
            }.into(),
            args: vec![
                IntermediateMemory{type_: AtomicTypeEnum::BOOL.into(), location: Location::new()}.into(),
                Integer{value: 43}.into()
            ]
        }.into(),
        FCV.clone() + MAV.clone() + BIV.clone() + MAV.clone();
        "user-defined fn-call"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
                Id::from("!"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::BOOL.into()],
                    Box::new(AtomicTypeEnum::BOOL.into())
                )
            ))),
            args: vec![
                IntermediateArg{type_: AtomicTypeEnum::BOOL.into(), location: Location::new()}.into(),
            ]
        }.into(),
        CodeVector::operator(Id::from("!")) + MAV.clone();
        "negation operator call"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
                Id::from("/"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ))),
            args: vec![
                IntermediateMemory{type_: AtomicTypeEnum::INT.into(), location: Location::new()}.into(),
                Integer{ value: -5}.into(),
            ]
        }.into(),
        CodeVector::operator(Id::from("/")) + BIV.clone() + MAV.clone();
        "division operator call"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
                Id::from("**"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ))),
            args: vec![
                Integer{ value: 21}.into(),
                IntermediateArg{type_: AtomicTypeEnum::INT.into(), location: Location::new()}.into(),
            ]
        }.into(),
        CodeVector::operator(Id::from("**")) + BIV.clone() + MAV.clone();
        "exponentiation operator call"
    )]
    #[test_case(
        IntermediateCtorCall{
            idx: 0,
            data: None,
            type_: IntermediateUnionType(
                vec![None, None]
            )
        }.into(),
        CCV.clone();
        "ctor call no data"
    )]
    #[test_case(
        IntermediateCtorCall{
            idx: 0,
            data: Some(Boolean{value: false}.into()),
            type_: IntermediateUnionType(
                vec![
                    Some(AtomicTypeEnum::INT.into()),
                    Some(AtomicTypeEnum::BOOL.into())
                ]
            )
        }.into(),
        CCV.clone() + BBV.clone();
        "ctor call with data"
    )]
    #[test_case(
        IntermediateLambda{
            args: Vec::new(),
            statements: Vec::new(),
            ret: Boolean{value: true}.into()
        }.into(),
        LV.clone();
        "lambda"
    )]
    fn test_expression_vector(expression: IntermediateExpression, expected_vector: CodeVector) {
        let vector = CodeVectorCalculator::expression_vector(&expression);
        assert_eq!(vector, expected_vector)
    }

    #[test_case(
        {
            let expression = IntermediateCtorCall{
                idx: 0,
                data: Some(Boolean{value: false}.into()),
                type_: IntermediateUnionType(
                    vec![
                        Some(AtomicTypeEnum::INT.into()),
                        Some(AtomicTypeEnum::BOOL.into())
                    ]
                )
            }.into();
            let statement_vector = AV.clone() + CodeVectorCalculator::expression_vector(&expression);
            (
                IntermediateAssignment{
                    expression: expression,
                    location: Location::new()
                }.into(),
                Ok(statement_vector)
            )
        };
        "assignment"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
            ];
            let target = Location::new();
            let branches = (
                vec![
                    IntermediateAssignment{
                        location: target.clone(),
                        expression: IntermediateValue::from(args[0].clone()).into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        location: target.clone(),
                        expression: IntermediateValue::from(args[1].clone()).into()
                    }.into(),
                ]
            );
            let condition = IntermediateMemory{
                location: Location::new(),
                type_: AtomicTypeEnum::BOOL.into()
            };
            let statement_vector = CodeVectorCalculator::statements_vector(&branches.0);
            let condition_vector = CodeVectorCalculator::value_vector(&condition.clone().into());
            (
                IntermediateIfStatement{
                    condition: condition.into(),
                    branches
                }.into(),
                Ok(statement_vector + condition_vector + ISV.clone())
            )
        };
        "if statement balanced"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
            ];
            let target = Location::new();
            let small_assignment = IntermediateAssignment{
                location: target.clone(),
                expression: IntermediateValue::from(args[0].clone()).into()
            };
            let large_assignment = IntermediateAssignment{
                expression: IntermediateFnCall{
                    fn_: BuiltInFn(
                        Id::from("*"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                    args: args.into_iter().map(IntermediateValue::from).collect_vec()
                }.into(),
                location: Location::new()
            };
            let large_final_assignment = IntermediateAssignment{
                expression: IntermediateValue::from(large_assignment.clone()).into(),
                location: target
            };
            let condition = IntermediateMemory{
                location: Location::new(),
                type_: AtomicTypeEnum::BOOL.into()
            };
            (
                IntermediateIfStatement{
                    condition: condition.into(),
                    branches: (
                        vec![small_assignment.into()],
                        vec![
                            large_assignment.into(),
                            large_final_assignment.into()
                        ],
                    )
                }.into(),
                Err(())
            )
        };
        "if statement unbalanced"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    location: Location::new(),
                    type_: AtomicTypeEnum::INT.into()
                }
            ];
            let target = Location::new();
            let assignment_0 = IntermediateAssignment{
                expression: args[0].clone().into(),
                location: target.clone()
            };
            let assignment_1 = IntermediateAssignment{
                expression: args[1].clone().into(),
                location: target.clone()
            };
            let subject = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateUnionType(
                    vec![Some(AtomicTypeEnum::INT.into()), Some(AtomicTypeEnum::INT.into())]
                ).into()
            };
            let statement_vector = CodeVectorCalculator::statement_vector(&assignment_0.clone().into());
            let subject_vector = CodeVectorCalculator::value_vector(&subject.clone().into());
            (
                IntermediateMatchStatement{
                    subject: subject.into(),
                    branches: vec![
                        IntermediateMatchBranch{
                            target: Some(args[0].clone()),
                            statements: vec![assignment_0.into()],
                        },
                        IntermediateMatchBranch{
                            target: Some(args[1].clone()),
                            statements: vec![assignment_1.into()],
                        },
                    ]
                }.into(),
                Ok(statement_vector + subject_vector + MSV.clone())
            )
        };
        "match statement balanced"
    )]
    #[test_case(
        {
            let medium_arg = IntermediateArg{
                location: Location::new(),
                type_: AtomicTypeEnum::INT.into()
            };
            let large_arg = IntermediateArg{
                location: Location::new(),
                type_: AtomicTypeEnum::INT.into()
            };
            let target = Location::new();
            let small_assignment = IntermediateAssignment{
                location: target.clone(),
                expression: IntermediateValue::from(Integer{value: 4}).into()
            };
            let medium_assignment = IntermediateAssignment{
                expression: medium_arg.clone().into(),
                location: target.clone()
            };
            let large_assignment = IntermediateAssignment{
                expression: IntermediateFnCall{
                    fn_: BuiltInFn(
                        Id::from("*"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                    args: vec![
                        Integer{value: 9}.into(),
                        large_arg.clone().into()
                    ]
                }.into(),
                location: target
            };
            let subject = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateUnionType(
                    vec![Some(AtomicTypeEnum::INT.into()), None, Some(AtomicTypeEnum::INT.into())]
                ).into()
            };
            (
                IntermediateMatchStatement{
                    subject: subject.into(),
                    branches: vec![
                        IntermediateMatchBranch{
                            target: None,
                            statements: vec![small_assignment.into()],
                        },
                        IntermediateMatchBranch{
                            target: Some(medium_arg),
                            statements: vec![medium_assignment.into()],
                        },
                        IntermediateMatchBranch{
                            target: Some(large_arg),
                            statements: vec![large_assignment.into()],
                        },
                    ]
                }.into(),
                Err(())
            )
        };
        "match statement unbalanced"
    )]
    fn test_statement_vector(statement_vector: (IntermediateStatement, Result<CodeVector, ()>)) {
        let (statement, result) = statement_vector;
        match result {
            Ok(expected_vector) => {
                let vector = CodeVectorCalculator::statement_vector(&statement);
                assert_eq!(vector, expected_vector)
            }
            Err(()) => {
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    CodeVectorCalculator::statement_vector(&statement)
                }));
                assert!(result.is_err())
            }
        }
    }

    #[test_case(
        (
            {
                let arg = IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    location: Location::new()
                };
                IntermediateLambda {
                    args: vec![arg.clone()],
                    statements: Vec::new(),
                    ret: arg.clone().into()
                }
            },
            Ok(MAV.clone())
        );
        "lambda no statements"
    )]
    #[test_case(
        {
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::BOOL.into(),
                location: Location::new()
            };
            let target = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            let statement = IntermediateIfStatement{
                condition: arg.clone().into(),
                branches:(
                    vec![
                        IntermediateAssignment{
                            location: target.location.clone(),
                            expression: IntermediateValue::from(Integer{ value: 1 }).into(),
                        }.into()
                    ],
                    vec![
                        IntermediateAssignment{
                            location: target.location.clone(),
                            expression: IntermediateValue::from(Integer{ value: -1 }).into(),
                        }.into()
                    ]
                )
            };
            let if_statement_vector = CodeVectorCalculator::if_statement_vector(&statement);
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    statements: vec![
                        statement.into()
                    ],
                    ret: target.into()
                },
                Ok(if_statement_vector + MAV.clone())
            )
        };
        "lambda if statement"
    )]
    #[test_case(
        {
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::BOOL.into(),
                location: Location::new()
            };
            let target = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            let statement = IntermediateIfStatement{
                condition: arg.clone().into(),
                branches:(
                    vec![
                        IntermediateAssignment{
                            location: target.location.clone(),
                            expression: IntermediateValue::from(Integer{ value: 1 }).into(),
                        }.into()
                    ],
                    vec![
                        IntermediateAssignment{
                            location: target.location.clone(),
                            expression: IntermediateValue::from(IntermediateMemory{
                                location: Location::new(),
                                type_: AtomicTypeEnum::INT.into()
                            }).into(),
                        }.into()
                    ]
                )
            };
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    statements: vec![
                        statement.into()
                    ],
                    ret: target.into()
                },
                Err(())
            )
        };
        "lambda if statement unbalanced"
    )]
    fn test_lambda_vector(lambda_vector: (IntermediateLambda, Result<CodeVector, ()>)) {
        let (lambda, result) = lambda_vector;
        match result {
            Ok(expected_vector) => {
                let mut vector = CodeVectorCalculator::lambda_vector(&lambda);
                vector.operators.retain(|_, v| *v != 0);
                assert_eq!(vector, expected_vector)
            }
            Err(()) => {
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    CodeVectorCalculator::lambda_vector(&lambda)
                }));
                assert!(result.is_err())
            }
        }
    }

    #[test]
    fn exhaustive_operator_test() {
        let lambda = IntermediateLambda {
            args: Vec::new(),
            statements: Vec::new(),
            ret: Integer { value: 0 }.into(),
        };
        let lambda_vector = CodeVectorCalculator::lambda_vector(&lambda);
        assert_eq!(
            lambda_vector
                .operators
                .keys()
                .cloned()
                .sorted()
                .collect_vec(),
            DEFAULT_CONTEXT.with(|context| context.keys().cloned().sorted().collect_vec())
        )
    }
}
