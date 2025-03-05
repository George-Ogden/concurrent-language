use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateLambda, IntermediateMemory, IntermediateStatement, IntermediateValue, Location,
};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
enum FnInst {
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

struct Inliner {}

impl Inliner {
    fn collect_fn_defs_from_statement(
        statement: &IntermediateStatement,
        fn_defs: &mut HashMap<Location, FnInst>,
    ) {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => match expression {
                IntermediateExpression::IntermediateLambda(lambda) => {
                    fn_defs.insert(location.clone(), lambda.clone().into());
                }
                IntermediateExpression::IntermediateValue(
                    IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::BuiltInFn(fn_)),
                ) => {
                    fn_defs.insert(location.clone(), fn_.clone().into());
                }
                IntermediateExpression::IntermediateValue(
                    IntermediateValue::IntermediateMemory(memory),
                ) if fn_defs.contains_key(&memory.location) => {
                    fn_defs.insert(location.clone(), memory.location.clone().into());
                }
                _ => {}
            },
            IntermediateStatement::IntermediateIfStatement(_) => todo!(),
            IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
        }
    }
    fn collect_fn_defs_from_statements(
        statements: &Vec<IntermediateStatement>,
        fn_defs: &mut HashMap<Location, FnInst>,
    ) {
        for statement in statements {
            Self::collect_fn_defs_from_statement(statement, fn_defs);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use lowering::{
        AtomicTypeEnum, BuiltInFn, Id, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateBuiltIn, IntermediateFnCall, IntermediateFnType, IntermediateValue, Location,
    };
    use test_case::test_case;

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
                HashMap::new()
            )
        };
        "no lambda defs"
    )]
    #[test_case(
        {
            let location = Location::new();
            let lambda = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            };
            (
                vec![
                    IntermediateAssignment {
                        location: location.clone(),
                        expression: lambda.clone().into()
                    }.into()
                ],
                HashMap::from([
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
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            };
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            let lambda_b = IntermediateLambda {
                args: vec![arg.clone()],
                statements: Vec::new(),
                ret: arg.clone().into()
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
                HashMap::from([
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
                HashMap::from([
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
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
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
                HashMap::from([
                    (memory.location.clone(), lambda.into()),
                    (location, memory.location.into()),
                ])
            )
        };
        "reassignment"
    )]
    fn test_collect_fn_defs(
        statements_fns: (Vec<IntermediateStatement>, HashMap<Location, FnInst>),
    ) {
        let (statements, expected_fn_defs) = statements_fns;
        let mut fn_defs = HashMap::new();
        Inliner::collect_fn_defs_from_statements(&statements, &mut fn_defs);
        assert_eq!(fn_defs, expected_fn_defs)
    }
}
