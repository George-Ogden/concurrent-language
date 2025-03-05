use itertools::Either::{self, Left, Right};
use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateLambda, IntermediateMemory, IntermediateStatement, IntermediateValue, Location,
};
use std::collections::HashMap;

type FnInst = Either<IntermediateLambda, BuiltInFn>;

struct Inliner {}

impl Inliner {
    fn collect_fn_defs_from_statement(
        statement: &IntermediateStatement,
    ) -> HashMap<Location, FnInst> {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => match expression {
                IntermediateExpression::IntermediateLambda(lambda) => {
                    HashMap::from([(location.clone(), Left(lambda.clone()))])
                }
                IntermediateExpression::IntermediateValue(
                    IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::BuiltInFn(fn_)),
                ) => HashMap::from([(location.clone(), Right(fn_.clone()))]),
                _ => HashMap::new(),
            },
            IntermediateStatement::IntermediateIfStatement(_) => todo!(),
            IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
        }
    }
    fn collect_fn_defs_from_statements(
        statements: &Vec<IntermediateStatement>,
    ) -> HashMap<Location, FnInst> {
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
                    (location, Left(lambda))
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
                    (location_a, Left(lambda_a)),
                    (location_b, Left(lambda_b)),
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
                    (location, Right(fn_))
                ])
            )
        };
        "built-in fn assignment"
    )]
    fn test_collect_fn_defs(
        statements_fns: (Vec<IntermediateStatement>, HashMap<Location, FnInst>),
    ) {
        let (statements, expected_fn_defs) = statements_fns;
        let fn_defs = Inliner::collect_fn_defs_from_statements(&statements);
        assert_eq!(fn_defs, expected_fn_defs)
    }
}
