use std::collections::HashMap;

use lowering::{BuiltInFn, Id, IntermediateBuiltIn, IntermediateExpression, IntermediateValue};
use once_cell::sync::Lazy;

struct CodeSizeConstants {
    builtin_bool_size: usize,
    builtin_int_size: usize,
    memory_access_size: usize,
    tuple_expression_size: usize,
    element_access_size: usize,
    value_expression_size: usize,
    fn_call_size: usize,
    ctor_call_size: usize,
    lambda_size: usize,
    operators: HashMap<Id, usize>,
}

const CODE_SIZE_CONSTANTS: Lazy<CodeSizeConstants> = Lazy::new(|| CodeSizeConstants {
    builtin_bool_size: 3,
    builtin_int_size: 8,
    memory_access_size: 38,
    tuple_expression_size: 2,
    element_access_size: 7,
    value_expression_size: 1,
    fn_call_size: 89,
    ctor_call_size: 92,
    lambda_size: 45,
    operators: HashMap::from(
        [
            ("**", 10),
            ("*", 4),
            ("/", 4),
            ("%", 4),
            ("+", 1),
            ("-", 1),
            (">>", 1),
            ("<<", 1),
            ("<=>", 2),
            ("&", 1),
            ("^", 1),
            ("|", 1),
            ("++", 1),
            ("--", 1),
            ("<", 1),
            ("<=", 1),
            (">", 1),
            (">=", 1),
            ("==", 1),
            ("!=", 1),
            ("!", 1),
        ]
        .map(|(id, size)| (Id::from(id), size as usize)),
    ),
});

struct CodeSizeEstimator {}

impl CodeSizeEstimator {
    fn builtin_size(built_in: &IntermediateBuiltIn) -> usize {
        match built_in {
            IntermediateBuiltIn::Integer(_) => CODE_SIZE_CONSTANTS.builtin_int_size,
            IntermediateBuiltIn::Boolean(_) => CODE_SIZE_CONSTANTS.builtin_bool_size,
            IntermediateBuiltIn::BuiltInFn(BuiltInFn(id, _)) => CODE_SIZE_CONSTANTS.operators[id],
        }
    }
    fn value_size(value: &IntermediateValue) -> usize {
        match value {
            IntermediateValue::IntermediateBuiltIn(built_in) => Self::builtin_size(built_in),
            IntermediateValue::IntermediateMemory(_) | IntermediateValue::IntermediateArg(_) => {
                CODE_SIZE_CONSTANTS.memory_access_size
            }
        }
    }
    fn values_size(values: &Vec<IntermediateValue>) -> usize {
        values.iter().map(Self::value_size).sum()
    }

    fn expression_size(expression: &IntermediateExpression) -> usize {
        let values_size = Self::values_size(&expression.values());
        match expression {
            IntermediateExpression::IntermediateValue(_) => {
                CODE_SIZE_CONSTANTS.value_expression_size + values_size
            }
            IntermediateExpression::IntermediateElementAccess(_) => {
                CODE_SIZE_CONSTANTS.element_access_size + values_size
            }
            IntermediateExpression::IntermediateTupleExpression(_) => {
                CODE_SIZE_CONSTANTS.tuple_expression_size + values_size
            }
            IntermediateExpression::IntermediateFnCall(_) => {
                CODE_SIZE_CONSTANTS.fn_call_size + values_size
            }
            IntermediateExpression::IntermediateCtorCall(_) => {
                CODE_SIZE_CONSTANTS.ctor_call_size + values_size
            }
            IntermediateExpression::IntermediateLambda(_) => CODE_SIZE_CONSTANTS.lambda_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, Id, Integer, IntermediateArg, IntermediateCtorCall,
        IntermediateElementAccess, IntermediateFnCall, IntermediateFnType, IntermediateLambda,
        IntermediateMemory, IntermediateTupleExpression, IntermediateTupleType,
        IntermediateUnionType, Location, DEFAULT_CONTEXT,
    };
    use test_case::test_case;

    const CSC: Lazy<CodeSizeConstants> = CODE_SIZE_CONSTANTS;
    const BBS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_bool_size);
    const BIS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_int_size);
    const MAS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.memory_access_size);
    const TES: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.tuple_expression_size);
    const EAS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.element_access_size);
    const VES: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.value_expression_size);
    const FCS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.fn_call_size);
    const CCS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.ctor_call_size);
    const LS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.lambda_size);

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
        CSC.operators[&Id::from("+")];
        "plus"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
            Id::from("=="),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::BOOL.into())
            )
        ))),
        CSC.operators[&Id::from("==")];
        "equality"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
            Id::from("/"),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            )
        ))),
        CSC.operators[&Id::from("/")];
        "division"
    )]
    #[test_case(
        IntermediateValue::from(IntermediateBuiltIn::from(BuiltInFn(
            Id::from("**"),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            )
        ))),
        CSC.operators[&Id::from("**")];
        "exponentiation"
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
        IntermediateLambda{
            args: Vec::new(),
            statements: Vec::new(),
            ret: Boolean{value: true}.into()
        }.into(),
        *LS;
        "lambda"
    )]
    fn test_expression_size(expression: IntermediateExpression, expected_size: usize) {
        let size = CodeSizeEstimator::expression_size(&expression);
        assert_eq!(size, expected_size)
    }
}
