use std::collections::HashMap;

use itertools::Itertools;
use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBlock, IntermediateBuiltIn,
    IntermediateExpression, IntermediateFnCall, IntermediateIf, IntermediateLambda,
    IntermediateMatch, IntermediateMatchBranch, IntermediateStatement, IntermediateValue,
};

use crate::code_size::{CodeVector, CODE_SIZE_CONSTANTS};

/// Compute code vectors for a program.
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

    fn expression_vector(expression: &IntermediateExpression) -> Result<CodeVector, String> {
        let values_vector = Self::values_vector(&expression.values());
        Ok(match expression {
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
            IntermediateExpression::IntermediateIf(if_) => Self::if_vector(if_)?,
            IntermediateExpression::IntermediateMatch(match_) => Self::match_vector(match_)?,
        })
    }

    fn statement_vector(statement: &IntermediateStatement) -> Result<CodeVector, String> {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::assignment_vector(assignment)
            }
        }
    }
    fn assignment_vector(assignment: &IntermediateAssignment) -> Result<CodeVector, String> {
        Ok(Self::expression_vector(&assignment.expression)? + CodeVector::assignment())
    }
    fn if_vector(if_statement: &IntermediateIf) -> Result<CodeVector, String> {
        let condition_vector = Self::value_vector(&if_statement.condition);
        let branch_vectors = (
            Self::block_vector(&if_statement.branches.0)?,
            Self::block_vector(&if_statement.branches.1)?,
        );
        if &branch_vectors.0 == &branch_vectors.1 {
            Ok(branch_vectors.0 + condition_vector + CodeVector::if_())
        } else {
            Err(String::from("If statement branches are not equal."))
        }
    }
    fn match_vector(match_statement: &IntermediateMatch) -> Result<CodeVector, String> {
        let subject_vector = Self::value_vector(&match_statement.subject);
        let branch_vectors = match_statement
            .branches
            .iter()
            .map(Self::match_branch_vector)
            .collect::<Result<Vec<_>, _>>()?;
        if branch_vectors.iter().all_equal() {
            Ok(branch_vectors[0].clone() + subject_vector + CodeVector::match_())
        } else {
            Err(String::from("Match statement branches are not equal."))
        }
    }
    fn match_branch_vector(match_branch: &IntermediateMatchBranch) -> Result<CodeVector, String> {
        Self::block_vector(&match_branch.block)
    }
    fn statements_vector(statements: &Vec<IntermediateStatement>) -> Result<CodeVector, String> {
        statements.iter().map(Self::statement_vector).sum()
    }
    fn block_vector(block: &IntermediateBlock) -> Result<CodeVector, String> {
        Ok(Self::statements_vector(&block.statements)? + Self::value_vector(&block.ret))
    }
    pub fn lambda_vector(lambda: &IntermediateLambda) -> Result<CodeVector, String> {
        let mut default = CodeVector::new();
        default.operators =
            HashMap::from_iter(CODE_SIZE_CONSTANTS.operators.keys().map(|k| (k.clone(), 0)));
        Ok(Self::block_vector(&lambda.block)? + default)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use itertools::Itertools;
    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, Id, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateCtorCall, IntermediateElementAccess, IntermediateFnCall, IntermediateFnType,
        IntermediateLambda, IntermediateMatchBranch, IntermediateMemory, IntermediateStatement,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateUnionType, Register, DEFAULT_CONTEXT,
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
    const IV: Lazy<CodeVector> = Lazy::new(|| CodeVector::if_());
    const MV: Lazy<CodeVector> = Lazy::new(|| CodeVector::match_());

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
            register: Register::new()
        }),
        MAV.clone();
        "argument"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateMemory{
            type_: AtomicTypeEnum::BOOL.into(),
            register: Register::new()
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
            IntermediateMemory{type_: AtomicTypeEnum::BOOL.into(), register: Register::new()}.into(),
            Integer{value: 43}.into()
        ]).into(),
        TEV.clone() + BIV.clone() + MAV.clone();
        "non-empty tuple"
    )]
    #[test_case(
        IntermediateElementAccess{
            idx: 1,
            value: IntermediateMemory{
                register: Register::new(),
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
                register: Register::new()
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
                register: Register::new()
            }.into(),
            args: vec![
                IntermediateMemory{type_: AtomicTypeEnum::BOOL.into(), register: Register::new()}.into(),
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
                IntermediateArg{type_: AtomicTypeEnum::BOOL.into(), register: Register::new()}.into(),
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
                IntermediateMemory{type_: AtomicTypeEnum::INT.into(), register: Register::new()}.into(),
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
                IntermediateArg{type_: AtomicTypeEnum::INT.into(), register: Register::new()}.into(),
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
            block: IntermediateBlock {
                statements: Vec::new(),
                ret: Boolean{value: true}.into()
            },
        }.into(),
        LV.clone();
        "lambda"
    )]
    fn test_expression_vector(expression: IntermediateExpression, expected_vector: CodeVector) {
        let vector = CodeVectorCalculator::expression_vector(&expression);
        assert_eq!(vector, Ok(expected_vector))
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
            let statement_vector = AV.clone() + CodeVectorCalculator::expression_vector(&expression).expect("");
            (
                IntermediateAssignment{
                    expression: expression,
                    register: Register::new()
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
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
            ];
            let target = Register::new();
            let values = (
                IntermediateValue::from(args[0].clone()),
                IntermediateValue::from(args[1].clone())
            );
            let condition = IntermediateMemory{
                register: Register::new(),
                type_: AtomicTypeEnum::BOOL.into()
            };
            let condition_vector = CodeVectorCalculator::value_vector(&condition.clone().into());
            let value_vector = CodeVectorCalculator::value_vector(&values.0.clone().into());
            (
                IntermediateAssignment {
                    expression: IntermediateIf{
                        condition: condition.into(),
                        branches: (
                            values.0.clone().into(),
                            values.1.clone().into(),
                        )
                    }.into(),
                    register: target
                }.into(),
                Ok(condition_vector + value_vector + AV.clone() + IV.clone())
            )
        };
        "if statement balanced"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
            ];
            let small_value = IntermediateValue::from(args[0].clone());
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
                register: Register::new()
            };
            let condition = IntermediateMemory{
                register: Register::new(),
                type_: AtomicTypeEnum::BOOL.into()
            };
            (
                IntermediateAssignment{
                    register: Register::new(),
                    expression: IntermediateIf{
                        condition: condition.into(),
                        branches: (
                            small_value.into(),
                            (
                                vec![
                                    large_assignment.clone().into(),
                                ],
                                large_assignment.into()
                            ).into()
                        )
                    }.into()
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
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                },
                IntermediateArg{
                    register: Register::new(),
                    type_: AtomicTypeEnum::INT.into()
                }
            ];
            let target = Register::new();
            let value_0 = IntermediateValue::from(args[0].clone());
            let value_1 = IntermediateValue::from(args[1].clone());
            let subject = IntermediateMemory{
                register: Register::new(),
                type_: IntermediateUnionType(
                    vec![Some(AtomicTypeEnum::INT.into()), Some(AtomicTypeEnum::INT.into())]
                ).into()
            };
            let statement_vector = CodeVectorCalculator::value_vector(&value_0.clone().into());
            let subject_vector = CodeVectorCalculator::value_vector(&subject.clone().into());
            (
                IntermediateAssignment {
                    register: target,
                    expression: IntermediateMatch {
                        subject: subject.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(args[0].clone()),
                                block: value_0.into(),
                            },
                            IntermediateMatchBranch{
                                target: Some(args[1].clone()),
                                block: value_1.into(),
                            },
                        ]
                    }.into()
                }.into(),
                Ok(AV.clone() + statement_vector + subject_vector + MV.clone())
            )
        };
        "match statement balanced"
    )]
    #[test_case(
        {
            let medium_arg = IntermediateArg{
                register: Register::new(),
                type_: AtomicTypeEnum::INT.into()
            };
            let large_arg = IntermediateArg{
                register: Register::new(),
                type_: AtomicTypeEnum::INT.into()
            };
            let target = Register::new();
            let small_value = IntermediateValue::from(Integer{value: -9});
            let medium_value = IntermediateValue::from(medium_arg.clone());
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
                register: target
            };
            let subject = IntermediateMemory{
                register: Register::new(),
                type_: IntermediateUnionType(
                    vec![Some(AtomicTypeEnum::INT.into()), None, Some(AtomicTypeEnum::INT.into())]
                ).into()
            };
            (
                IntermediateAssignment {
                    register: Register::new(),
                    expression: IntermediateMatch {
                        subject: subject.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                block: small_value.into(),
                            },
                            IntermediateMatchBranch{
                                target: Some(medium_arg),
                                block: medium_value.into(),
                            },
                            IntermediateMatchBranch{
                                target: Some(large_arg),
                                block: (
                                    vec![large_assignment.clone().into()],
                                    large_assignment.clone().into()
                                ).into()
                            },
                        ]
                    }.into()
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
                let vector = CodeVectorCalculator::statement_vector(&statement)
                    .expect("Expected code vector");
                assert_eq!(vector, expected_vector)
            }
            Err(()) => {
                let result = CodeVectorCalculator::statement_vector(&statement);
                assert!(result.is_err())
            }
        }
    }

    #[test_case(
        (
            {
                let arg = IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    register: Register::new()
                };
                IntermediateLambda {
                    args: vec![arg.clone()],
                    block: IntermediateBlock {
                        statements: Vec::new(),
                        ret: arg.clone().into()
                    },
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
                register: Register::new()
            };
            let target = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            let assignment = IntermediateAssignment{
                register: target.register.clone(),
                expression: IntermediateIf {
                    condition: arg.clone().into(),
                    branches: (
                        IntermediateValue::from(Integer{ value: 1 }).into(),
                        IntermediateValue::from(Integer{ value: -1 }).into(),
                    )
                }.into()
            };
            let if_statement_vector = CodeVectorCalculator::assignment_vector(&assignment).expect("");
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    block: IntermediateBlock{
                        statements: vec![
                            assignment.into()
                        ],
                        ret: target.into()
                    },
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
                register: Register::new()
            };
            let target = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            let assignment = IntermediateAssignment{
                register: target.register.clone(),
                expression: IntermediateIf {
                    condition: arg.clone().into(),
                    branches: (
                        IntermediateValue::from(Integer{ value: 1 }).into(),
                        IntermediateValue::from(IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT))).into(),
                    )
                }.into()
            };
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    block: IntermediateBlock{
                        statements: vec![
                            assignment.into()
                        ],
                        ret: target.into()
                    },
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
                let mut vector =
                    CodeVectorCalculator::lambda_vector(&lambda).expect("Expected code vector");
                vector.operators.retain(|_, v| *v != 0);
                assert_eq!(vector, expected_vector)
            }
            Err(()) => {
                let result = CodeVectorCalculator::lambda_vector(&lambda);
                assert!(result.is_err())
            }
        }
    }

    #[test]
    fn exhaustive_operator_test() {
        let lambda = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: Vec::new(),
                ret: Integer { value: 0 }.into(),
            },
        };
        let lambda_vector = CodeVectorCalculator::lambda_vector(&lambda).expect("");
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
