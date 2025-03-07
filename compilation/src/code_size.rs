use std::collections::HashMap;
use std::iter::Sum;
use std::ops::{Add, Mul};

use gcollections::ops::*;
use interval::ops::*;
use interval::Interval;
use itertools::Itertools;
use lowering::{
    BuiltInFn, IBlock, IIf, ILambda, IMatch, Id, IntermediateAssignment, IntermediateBuiltIn,
    IntermediateExpression, IntermediateFnCall, IntermediateMatchBranch, IntermediateStatement,
    IntermediateValue,
};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;

use crate::define_named_vector;

define_named_vector!(
    CodeVector,
    builtin_bool,
    builtin_int,
    builtin_fn,
    memory_access,
    tuple_expression,
    element_access,
    value_expression,
    fn_call,
    ctor_call,
    lambda,
    assignment,
    if_,
    match_,
);

pub const CODE_SIZE_CONSTANTS: Lazy<CodeVector> = Lazy::new(|| CodeVector {
    builtin_bool: 0,
    builtin_int: 0,
    builtin_fn: 0,
    memory_access: 0,
    tuple_expression: 7,
    element_access: 0,
    value_expression: 0,
    fn_call: 98,
    ctor_call: 14,
    lambda: 0,
    assignment: 0,
    if_: 8,
    match_: 8,
    operators: HashMap::from(
        [
            ("**", 12),
            ("*", 9),
            ("/", 10),
            ("%", 10),
            ("+", 9),
            ("-", 9),
            (">>", 9),
            ("<<", 9),
            ("<=>", 9),
            ("&", 9),
            ("^", 9),
            ("|", 9),
            ("++", 8),
            ("--", 8),
            ("<", 9),
            ("<=", 9),
            (">", 9),
            (">=", 9),
            ("==", 9),
            ("!=", 9),
            ("!", 8),
        ]
        .map(|(id, size)| (Id::from(id), size as usize)),
    ),
});

pub struct CodeSizeEstimator {}

impl CodeSizeEstimator {
    fn builtin_size(built_in: &IntermediateBuiltIn) -> usize {
        match built_in {
            IntermediateBuiltIn::Integer(_) => CODE_SIZE_CONSTANTS.builtin_int,
            IntermediateBuiltIn::Boolean(_) => CODE_SIZE_CONSTANTS.builtin_bool,
            IntermediateBuiltIn::BuiltInFn(_) => CODE_SIZE_CONSTANTS.builtin_fn,
        }
    }
    fn value_size(value: &IntermediateValue) -> usize {
        match value {
            IntermediateValue::IntermediateBuiltIn(built_in) => Self::builtin_size(built_in),
            IntermediateValue::IntermediateMemory(_) | IntermediateValue::IntermediateArg(_) => {
                CODE_SIZE_CONSTANTS.memory_access
            }
        }
    }
    fn values_size(values: &Vec<IntermediateValue>) -> usize {
        values.iter().map(Self::value_size).sum()
    }

    fn expression_size(expression: &IntermediateExpression) -> Interval<usize> {
        let values_size = Self::values_size(&expression.values());
        match expression {
            IntermediateExpression::IntermediateValue(_) => {
                Interval::singleton(CODE_SIZE_CONSTANTS.value_expression + values_size)
            }
            IntermediateExpression::IntermediateElementAccess(_) => {
                Interval::singleton(CODE_SIZE_CONSTANTS.element_access + values_size)
            }
            IntermediateExpression::IntermediateTupleExpression(_) => {
                Interval::singleton(CODE_SIZE_CONSTANTS.tuple_expression + values_size)
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                fn_:
                    IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::BuiltInFn(BuiltInFn(
                        id,
                        _,
                    ))),
                args,
            }) => Interval::singleton(CODE_SIZE_CONSTANTS.operators[id] + Self::values_size(args)),
            IntermediateExpression::IntermediateFnCall(_) => {
                Interval::singleton(CODE_SIZE_CONSTANTS.fn_call + values_size)
            }
            IntermediateExpression::IntermediateCtorCall(_) => {
                Interval::singleton(CODE_SIZE_CONSTANTS.ctor_call + values_size)
            }
            IntermediateExpression::ILambda(_) => Interval::singleton(CODE_SIZE_CONSTANTS.lambda),
            IntermediateExpression::IIf(if_) => Self::if_size(if_),
            IntermediateExpression::IMatch(match_) => Self::match_size(match_),
        }
    }

    fn statement_size(statement: &IntermediateStatement) -> Interval<usize> {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::assignment_size(assignment)
            }
        }
    }
    fn assignment_size(assignment: &IntermediateAssignment) -> Interval<usize> {
        Self::expression_size(&assignment.expression) + CODE_SIZE_CONSTANTS.assignment
    }
    fn if_size(if_: &IIf) -> Interval<usize> {
        let condition_size = Self::value_size(&if_.condition);
        let branch_sizes =
            Self::block_size(&if_.branches.0).hull(&Self::block_size(&if_.branches.1));
        branch_sizes + condition_size + CODE_SIZE_CONSTANTS.if_
    }
    fn match_size(match_: &IMatch) -> Interval<usize> {
        let subject_size = Self::value_size(&match_.subject);
        let branch_sizes = match_
            .branches
            .iter()
            .map(Self::match_branch_size)
            .reduce(|x, y| Interval::hull(&x, &y))
            .unwrap();
        branch_sizes + subject_size + CODE_SIZE_CONSTANTS.match_
    }
    fn match_branch_size(match_branch: &IntermediateMatchBranch) -> Interval<usize> {
        Self::block_size(&match_branch.block)
    }
    fn statements_size(statements: &Vec<IntermediateStatement>) -> Interval<usize> {
        statements
            .iter()
            .map(Self::statement_size)
            .fold(Interval::singleton(0), Interval::add)
    }
    fn block_size(block: &IBlock) -> Interval<usize> {
        Self::statements_size(&block.statements) + Self::value_size(&block.ret)
    }
    pub fn estimate_size(lambda: &ILambda) -> (usize, usize) {
        let size_interval = Self::block_size(&lambda.block);
        (size_interval.lower(), size_interval.upper())
    }
}

#[cfg(test)]
mod tests {
    use std::{cmp::min, collections::HashSet};

    use super::*;

    use interval::Interval;
    use itertools::Itertools;
    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, ILambda, Id, Integer, IntermediateArg,
        IntermediateAssignment, IntermediateCtorCall, IntermediateElementAccess,
        IntermediateFnCall, IntermediateFnType, IntermediateMatchBranch, IntermediateMemory,
        IntermediateStatement, IntermediateTupleExpression, IntermediateTupleType,
        IntermediateUnionType, Location, DEFAULT_CONTEXT,
    };
    use test_case::test_case;

    const CSC: Lazy<CodeVector> = CODE_SIZE_CONSTANTS;
    const BBS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_bool);
    const BIS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_int);
    const BFS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_fn);
    const MAS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.memory_access);
    const TES: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.tuple_expression);
    const EAS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.element_access);
    const VES: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.value_expression);
    const FCS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.fn_call);
    const CCS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.ctor_call);
    const LS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.lambda);
    const AS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.assignment);
    const IS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.if_);
    const MS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.match_);

    #[test]
    fn exhaustive_operator_test() {
        assert_eq!(
            CSC.operators.keys().cloned().collect::<HashSet<_>>(),
            DEFAULT_CONTEXT.with(|context| context.keys().cloned().collect::<HashSet<_>>())
        )
    }

    #[test_case(
        IntermediateValue::from(Boolean{value: true}),
        *BBS;
        "bool"
    )]
    #[test_case(
        IntermediateValue::from(Integer{value: 5}),
        *BIS;
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
        *BFS;
        "built-in function"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateArg{
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new()
        }),
        *MAS;
        "argument"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateMemory{
            type_: AtomicTypeEnum::BOOL.into(),
            location: Location::new()
        }),
        *MAS;
        "memory"
    )]
    fn test_value_size(value: IntermediateValue, expected_size: usize) {
        let size = CodeSizeEstimator::value_size(&value);
        assert_eq!(size, expected_size)
    }

    #[test_case(
        IntermediateTupleExpression(Vec::new()).into(),
        *TES;
        "empty tuple"
    )]
    #[test_case(
        IntermediateTupleExpression(vec![
            IntermediateMemory{type_: AtomicTypeEnum::BOOL.into(), location: Location::new()}.into(),
            Integer{value: 43}.into()
        ]).into(),
        *TES + *BIS + *MAS;
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
        *EAS + *MAS;
        "tuple access"
    )]
    #[test_case(
        IntermediateValue::from(Integer{value: -27}).into(),
        *VES + *BIS;
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
        *FCS + *MAS;
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
        *FCS + *MAS + *BIS + *MAS;
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
        CSC.operators[&Id::from("!")] + *MAS;
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
        CSC.operators[&Id::from("/")] + *BIS + *MAS;
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
        CSC.operators[&Id::from("**")] + *BIS + *MAS;
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
        *CCS;
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
        *CCS + *BBS;
        "ctor call with data"
    )]
    #[test_case(
        ILambda{
            args: Vec::new(),
            block: IBlock {
                statements: Vec::new(),
                ret: Boolean{value: true}.into()
            },
        }.into(),
        *LS;
        "lambda"
    )]
    fn test_expression_size(expression: IntermediateExpression, expected_size: usize) {
        let size = CodeSizeEstimator::expression_size(&expression);
        assert_eq!(size, Interval::singleton(expected_size))
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
            let statement_size = CodeSizeEstimator::expression_size(&expression) + *AS;
            (
                IntermediateAssignment{
                    expression: expression,
                    location: Location::new()
                }.into(),
                statement_size
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
                location: Location::new()
            };
            let condition = IntermediateMemory{
                location: Location::new(),
                type_: AtomicTypeEnum::BOOL.into()
            };
            let small_statement_size = CodeSizeEstimator::value_size(&small_value.clone().into()).lower();
            let large_statements_size = CodeSizeEstimator::statement_size(&large_assignment.clone().into()).lower() + CodeSizeEstimator::value_size(&large_assignment.clone().into()).lower();
            let condition_size = CodeSizeEstimator::value_size(&condition.clone().into());
            let (lower_bound, upper_bound) = (small_statement_size + condition_size + *IS, large_statements_size + condition_size + *IS);
            (
                IntermediateAssignment {
                    location: Location::new(),
                    expression: IIf{
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
                    }.into(),
                }.into(),
                Interval::new(lower_bound, upper_bound)
            )
        };
        "if statement"
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
            let small_expression = IntermediateValue::from(Integer{value: 4});
            let medium_expression = IntermediateValue::from(medium_arg.clone());
            let large_expression: IntermediateAssignment = IntermediateExpression::from(IntermediateFnCall{
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
            }).into();
            let subject = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateUnionType(
                    vec![Some(AtomicTypeEnum::INT.into()), None, Some(AtomicTypeEnum::INT.into())]
                ).into()
            };
            let small_expression_size = CodeSizeEstimator::expression_size(&small_expression.clone().into()).lower();
            let medium_expression_size = CodeSizeEstimator::expression_size(&medium_expression.clone().into()).lower();
            let large_expression_size = CodeSizeEstimator::assignment_size(&large_expression.clone().into()).lower() + CodeSizeEstimator::value_size(&large_expression.clone().into());
            let subject_size = CodeSizeEstimator::value_size(&subject.clone().into());
            let (lower_bound, upper_bound) = (min(small_expression_size, medium_expression_size) + subject_size + *MS, large_expression_size + subject_size + *MS);
            (
                IntermediateAssignment {
                    location: Location::new(),
                    expression: IMatch {
                        subject: subject.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                block: small_expression.into(),
                            },
                            IntermediateMatchBranch{
                                target: Some(medium_arg),
                                block: medium_expression.into(),
                            },
                            IntermediateMatchBranch{
                                target: Some(large_arg),
                                block: IBlock {
                                    statements: vec![large_expression.clone().into()],
                                    ret: large_expression.clone().into()
                                }
                            },
                        ]
                    }.into()
                }.into(),
                Interval::new(lower_bound, upper_bound)
            )
        };
        "match statement"
    )]
    fn test_statement_size(statement_size: (IntermediateStatement, Interval<usize>)) {
        let (statement, expected_size) = statement_size;
        let size = CodeSizeEstimator::statement_size(&statement);
        assert_eq!(size, expected_size)
    }

    #[test_case(
        (
            {
                let arg = IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    location: Location::new()
                };
                ILambda {
                    args: vec![arg.clone()],
                    block: IBlock {
                        statements: Vec::new(),
                        ret: arg.clone().into()
                    },
                }.into()
            },
            (*MAS, *MAS)
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
            let statement = IntermediateAssignment {
                location: target.location.clone(),
                expression: IIf{
                    condition: arg.clone().into(),
                    branches:(
                        IntermediateValue::from(Integer{ value: 1 }).into(),
                        IntermediateValue::from(IntermediateMemory{
                            location: Location::new(),
                            type_: AtomicTypeEnum::INT.into()
                        }).into(),
                    )
                }.into()
            };
            let range = CodeSizeEstimator::statement_size(&statement.clone().into());
            let lower_bound = range.lower() + *MAS;
            let upper_bound = range.upper() + *MAS;
            (
                ILambda {
                    args: vec![arg.clone()],
                    block: IBlock{
                        statements: vec![
                            statement.into()
                        ],
                        ret: target.into()
                    }
                }.into(),
                (lower_bound, upper_bound)
            )
        };
        "lambda if statement"
    )]
    fn test_lambda_size(lambda_size: (ILambda, (usize, usize))) {
        let (lambda, expected_size) = lambda_size;
        let size = CodeSizeEstimator::estimate_size(&lambda);
        assert_eq!(size, expected_size)
    }
}
