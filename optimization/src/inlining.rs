use lowering::{
    IntermediateAssignment, IntermediateLambda, IntermediateMemory, IntermediateStatement, Location,
};
use std::collections::HashMap;

struct Inliner {}

impl Inliner {
    fn collect_fn_defs_from_statement(
        statement: &IntermediateStatement,
    ) -> HashMap<Location, IntermediateLambda> {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => match expression {
                lowering::IntermediateExpression::IntermediateLambda(lambda) => {
                    HashMap::from([(location.clone(), lambda.clone())])
                }
                _ => HashMap::new(),
            },
            IntermediateStatement::IntermediateIfStatement(_) => todo!(),
            IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
        }
    }
    fn collect_fn_defs_from_statements(
        statements: &Vec<IntermediateStatement>,
    ) -> HashMap<Location, IntermediateLambda> {
        statements
            .iter()
            .flat_map(Self::collect_fn_defs_from_statement)
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use lowering::{
        AtomicTypeEnum, BuiltInFn, Id, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateBuiltIn, IntermediateFnCall, IntermediateFnType, Location,
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
                    (location, lambda)
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
                    (location_a, lambda_a),
                    (location_b, lambda_b),
                ])
            )
        };
        "multiple lambda defs"
    )]
    fn test_collect_fn_defs(
        statements_fns: (
            Vec<IntermediateStatement>,
            HashMap<Location, IntermediateLambda>,
        ),
    ) {
        let (statements, expected_fn_defs) = statements_fns;
        let fn_defs = Inliner::collect_fn_defs_from_statements(&statements);
        assert_eq!(fn_defs, expected_fn_defs)
    }
}
