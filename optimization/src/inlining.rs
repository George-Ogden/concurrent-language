use counter::Counter;
use itertools::Itertools;
use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateIfStatement, IntermediateLambda, IntermediateStatement, IntermediateValue,
    Location,
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

type FnDefs = HashMap<Location, FnInst>;

struct Inliner {}

impl Inliner {
    fn collect_fn_defs_from_statement(statement: &IntermediateStatement, fn_defs: &mut FnDefs) {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::collect_fns_defs_from_assignment(assignment, fn_defs)
            }
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                Self::collect_fn_defs_from_if_statement(if_statement, fn_defs)
            }
            IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
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
            _ => {}
        }
    }
    fn collect_fn_defs_from_if_statement(
        IntermediateIfStatement {
            condition: _,
            branches,
        }: &IntermediateIfStatement,
        fn_defs: &mut FnDefs,
    ) {
        let mut branch_fn_defs = (fn_defs.clone(), fn_defs.clone());
        Self::collect_fn_defs_from_statements(&branches.0, &mut branch_fn_defs.0);
        Self::collect_fn_defs_from_statements(&branches.1, &mut branch_fn_defs.1);
        fn_defs.extend(Self::merge_fn_defs(vec![
            branch_fn_defs.0,
            branch_fn_defs.1,
        ]))
    }
    fn collect_fn_defs_from_statements(
        statements: &Vec<IntermediateStatement>,
        fn_defs: &mut FnDefs,
    ) {
        for statement in statements {
            Self::collect_fn_defs_from_statement(statement, fn_defs);
        }
    }
    fn merge_fn_defs(fn_defs: Vec<FnDefs>) -> FnDefs {
        let counter = fn_defs
            .iter()
            .flat_map(HashMap::keys)
            .collect::<Counter<_>>();
        let keys = counter
            .into_iter()
            .filter_map(|(key, count)| if count == 1 { Some(key.clone()) } else { None })
            .collect_vec();
        let combined = fn_defs.into_iter().flatten().collect::<HashMap<_, _>>();
        keys.into_iter()
            .map(|k| (k.clone(), combined[&k].clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use lowering::{
        AtomicTypeEnum, BuiltInFn, Id, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateBuiltIn, IntermediateFnCall, IntermediateFnType, IntermediateIfStatement,
        IntermediateMemory, IntermediateValue, Location,
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
            let location_0 = Location::new();
            let location_1 = Location::new();
            let lambda_0 = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            };
            let lambda_1 = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 13}.into()
            };
            (
                vec![
                    IntermediateIfStatement {
                        condition: IntermediateArg{
                            location: Location::new(),
                            type_: AtomicTypeEnum::BOOL.into()
                        }.into(),
                        branches: (
                            vec![
                                IntermediateAssignment {
                                    location: location_0.clone(),
                                    expression: lambda_0.clone().into()
                                }.into()
                            ],
                            vec![
                                IntermediateAssignment {
                                    location: location_1.clone(),
                                    expression: lambda_1.clone().into()
                                }.into()
                            ]
                        )
                    }.into(),
                ],
                FnDefs::from([
                    (location_0, lambda_0.into()),
                    (location_1, lambda_1.into()),
                ])
            )
        };
        "if statement single appearances"
    )]
    #[test_case(
        {
            let location = Location::new();
            let lambda_0 = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            };
            let lambda_1 = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 13}.into()
            };
            (
                vec![
                    IntermediateIfStatement {
                        condition: IntermediateArg{
                            location: Location::new(),
                            type_: AtomicTypeEnum::BOOL.into()
                        }.into(),
                        branches: (
                            vec![
                                IntermediateAssignment {
                                    location: location.clone(),
                                    expression: lambda_0.clone().into()
                                }.into()
                            ],
                            vec![
                                IntermediateAssignment {
                                    location: location.clone(),
                                    expression: lambda_1.clone().into()
                                }.into()
                            ]
                        )
                    }.into(),
                ],
                FnDefs::new()
            )
        };
        "if statement double appearance"
    )]
    fn test_collect_fn_defs(statements_fns: (Vec<IntermediateStatement>, FnDefs)) {
        let (statements, expected_fn_defs) = statements_fns;
        let mut fn_defs = FnDefs::new();
        Inliner::collect_fn_defs_from_statements(&statements, &mut fn_defs);
        assert_eq!(fn_defs, expected_fn_defs)
    }
}
