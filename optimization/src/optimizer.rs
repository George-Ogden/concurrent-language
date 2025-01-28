use std::collections::HashSet;

use lowering::{IntermediateArg, IntermediateExpression, Location};

struct Optimizer {
    used_locations: HashSet<Location>,
    used_args: HashSet<IntermediateArg>,
}

impl Optimizer {
    fn new() -> Self {
        Optimizer {
            used_locations: HashSet::new(),
            used_args: HashSet::new(),
        }
    }
    fn find_used_values(&mut self, expression: IntermediateExpression) {
        let values = expression.values();
        for value in values {
            match value {
                lowering::IntermediateValue::IntermediateMemory(location) => {
                    self.used_locations.insert(location);
                }
                lowering::IntermediateValue::IntermediateArg(arg) => {
                    self.used_args.insert(arg);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, Id, Integer, IntermediateArg, IntermediateBuiltIn,
        IntermediateFnType, IntermediateType, IntermediateValue,
    };
    use test_case::test_case;

    #[test_case(
        (
            IntermediateExpression::from(IntermediateValue::from(
                IntermediateBuiltIn::from(Integer{
                    value: 8
                })
            )),
            Vec::new(),
            Vec::new(),
        );
        "integer"
    )]
    #[test_case(
        (
            IntermediateExpression::from(IntermediateValue::from(
                IntermediateBuiltIn::from(Boolean{
                    value: false
                })
            )),
            Vec::new(),
            Vec::new(),
        );
        "boolean"
    )]
    #[test_case(
        (
            IntermediateExpression::from(IntermediateValue::from(
                IntermediateBuiltIn::BuiltInFn(
                    Id::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                )
            )),
            Vec::new(),
            Vec::new(),
        );
        "built-in fn"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateExpression::from(IntermediateValue::from(
                    location.clone()
                )),
                vec![location.clone()],
                Vec::new(),
            )
        };
        "memory location"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            (
                IntermediateExpression::from(IntermediateValue::from(
                    arg.clone()
                )),
                Vec::new(),
                vec![arg.clone()],
            )
        };
        "arg"
    )]
    fn test_find_used_values(
        expression_locations_args: (IntermediateExpression, Vec<Location>, Vec<IntermediateArg>),
    ) {
        let (expression, expected_locations, expected_args) = expression_locations_args;
        let mut optimizer = Optimizer::new();
        optimizer.find_used_values(expression);
        assert_eq!(
            optimizer.used_locations,
            HashSet::from_iter(expected_locations)
        );
        assert_eq!(optimizer.used_args, HashSet::from_iter(expected_args));
    }
}
