use std::collections::HashMap;

use lowering::{BuiltInFn, Id, IntermediateBuiltIn, IntermediateValue};
use once_cell::sync::Lazy;

struct CodeSizeConstants {
    builtin_bool_size: usize,
    builtin_int_size: usize,
    operators: HashMap<Id, usize>,
}

const CODE_SIZE_CONSTANTS: Lazy<CodeSizeConstants> = Lazy::new(|| CodeSizeConstants {
    builtin_bool_size: 3,
    builtin_int_size: 8,
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
    fn value_size(value: &IntermediateValue) -> usize {
        match value {
            IntermediateValue::IntermediateBuiltIn(built_in) => Self::builtin_size(built_in),
            IntermediateValue::IntermediateMemory(intermediate_memory) => todo!(),
            IntermediateValue::IntermediateArg(intermediate_arg) => todo!(),
        }
    }

    fn builtin_size(built_in: &IntermediateBuiltIn) -> usize {
        match built_in {
            IntermediateBuiltIn::Integer(_) => CODE_SIZE_CONSTANTS.builtin_int_size,
            IntermediateBuiltIn::Boolean(_) => CODE_SIZE_CONSTANTS.builtin_bool_size,
            IntermediateBuiltIn::BuiltInFn(BuiltInFn(id, _)) => CODE_SIZE_CONSTANTS.operators[id],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, Id, Integer, IntermediateFnType, DEFAULT_CONTEXT,
    };
    use test_case::test_case;

    const CSC: Lazy<CodeSizeConstants> = CODE_SIZE_CONSTANTS;
    const BBS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_bool_size);
    const BIS: Lazy<usize> = Lazy::new(|| CODE_SIZE_CONSTANTS.builtin_int_size);

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
    fn test_value_size(value: IntermediateValue, expected_size: usize) {
        let size = CodeSizeEstimator::value_size(&value);
        assert_eq!(size, expected_size)
    }
}
