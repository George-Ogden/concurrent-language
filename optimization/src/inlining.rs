use counter::Counter;
use itertools::Itertools;
use lowering::{
    BuiltInFn, IntermediateAssignment, IntermediateBuiltIn, IntermediateExpression,
    IntermediateFnCall, IntermediateIfStatement, IntermediateLambda, IntermediateMatchBranch,
    IntermediateMatchStatement, IntermediateMemory, IntermediateProgram, IntermediateStatement,
    IntermediateValue, Location,
};
use std::{collections::HashMap, convert::identity};

use crate::{
    dead_code_analysis::DeadCodeAnalyzer,
    equivalent_expression_elimination::EquivalentExpressionEliminator, refresher::Refresher,
};
use compilation::CodeSizeEstimator;

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

pub struct Inliner {
    fn_defs: FnDefs,
    size_limit: usize,
}

const MAX_INLINING_ITERATIONS: usize = 1000;

impl Inliner {
    pub fn inline_up_to_size(
        program: IntermediateProgram,
        size_limit: Option<usize>,
    ) -> IntermediateProgram {
        let mut should_continue = true;
        let mut program = program;
        let mut i = 0;
        while should_continue && i < MAX_INLINING_ITERATIONS {
            (program.main, should_continue) = Inliner::inline_iteration(program.main, size_limit);
            program = EquivalentExpressionEliminator::eliminate_equivalent_expressions(program);
            program = DeadCodeAnalyzer::remove_dead_code(program);
            i += 1;
        }
        program
    }
    fn new() -> Self {
        Inliner {
            fn_defs: FnDefs::new(),
            size_limit: usize::max_value(),
        }
    }

    fn collect_fn_defs_from_statement(statement: &IntermediateStatement, fn_defs: &mut FnDefs) {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                Self::collect_fns_defs_from_assignment(assignment, fn_defs)
            }
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                Self::collect_fn_defs_from_if_statement(if_statement, fn_defs)
            }
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                Self::collect_fn_defs_from_match_statement(match_statement, fn_defs);
            }
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
                Self::collect_fn_defs_from_statements(&lambda.statements, fn_defs);
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
    fn collect_fn_defs_from_match_statement(
        IntermediateMatchStatement {
            subject: _,
            branches,
        }: &IntermediateMatchStatement,
        fn_defs: &mut FnDefs,
    ) {
        let mut branch_fn_defs = branches
            .iter()
            .map(|branch| {
                let mut fn_defs = fn_defs.clone();
                Self::collect_fn_defs_from_statements(&branch.statements, &mut fn_defs);
                fn_defs
            })
            .collect_vec();
        if branches.len() == 1 {
            let statements = &branches[0].statements.clone();
            if let Some(IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression: _,
                location,
            })) = &statements.get(statements.len() - 1)
            {
                branch_fn_defs[0].remove(location);
            }
        }
        fn_defs.extend(Self::merge_fn_defs(branch_fn_defs));
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

    fn inline(
        &self,
        mut lambda: IntermediateLambda,
        args: Vec<IntermediateValue>,
    ) -> (Vec<IntermediateStatement>, IntermediateValue) {
        Refresher::refresh_for_inlining(&mut lambda);
        let assignments = lambda
            .args
            .iter()
            .zip_eq(args.into_iter())
            .map(|(arg, v)| {
                IntermediateAssignment {
                    location: arg.location.clone(),
                    expression: v.into(),
                }
                .into()
            })
            .collect_vec();
        let mut statements = assignments;
        statements.extend(lambda.statements);
        (statements, lambda.ret)
    }

    fn inline_iteration(
        lambda: IntermediateLambda,
        size_limit: Option<usize>,
    ) -> (IntermediateLambda, bool) {
        let bounds = CodeSizeEstimator::estimate_size(&lambda);
        if let Some(size) = size_limit {
            if bounds.1 >= size {
                return (lambda, false);
            }
        }
        let IntermediateLambda {
            args,
            statements,
            ret,
        } = lambda;
        let mut inliner = Inliner::from(&statements);
        if let Some(size) = size_limit {
            inliner.size_limit = size;
        }
        let inliner = inliner;
        let (statements, should_continue) = inliner.inline_statements(statements);
        (
            IntermediateLambda {
                args,
                statements,
                ret,
            },
            should_continue,
        )
    }
    fn inline_statements(
        &self,
        statements: Vec<IntermediateStatement>,
    ) -> (Vec<IntermediateStatement>, bool) {
        let (statements, continues): (Vec<_>, Vec<_>) = statements
            .into_iter()
            .map(|statement| self.inline_statement(statement))
            .unzip();
        (statements.concat(), continues.into_iter().any(identity))
    }
    fn inline_statement(
        &self,
        statement: IntermediateStatement,
    ) -> (Vec<IntermediateStatement>, bool) {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                self.inline_assignment(assignment)
            }
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                self.inline_if_statement(if_statement)
            }
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                self.inline_match_statement(match_statement)
            }
        }
    }
    fn inline_assignment(
        &self,
        IntermediateAssignment {
            expression,
            location,
        }: IntermediateAssignment,
    ) -> (Vec<IntermediateStatement>, bool) {
        let mut should_continue = false;
        let mut statements = Vec::new();
        let expression = match expression {
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                fn_: IntermediateValue::IntermediateMemory(IntermediateMemory { type_, location }),
                args,
            }) if self.fn_defs.contains_key(&location) => {
                let mut fn_def = self.fn_defs.get(&location);
                while let Some(FnInst::Ref(reference)) = fn_def {
                    fn_def = self.fn_defs.get(&reference)
                }
                match fn_def {
                    Some(FnInst::Lambda(lambda))
                        if CodeSizeEstimator::estimate_size(lambda).1 < self.size_limit =>
                    {
                        let (extra_statements, value) = self.inline(lambda.clone(), args);
                        statements = extra_statements;
                        should_continue = true;
                        value.into()
                    }
                    Some(FnInst::BuiltIn(built_in_fn)) => IntermediateFnCall {
                        fn_: built_in_fn.clone().into(),
                        args,
                    }
                    .into(),
                    Some(FnInst::Ref(_)) => panic!("Determined that fn_def was not reference."),
                    _ => IntermediateFnCall {
                        fn_: IntermediateMemory { type_, location }.into(),
                        args,
                    }
                    .into(),
                }
            }
            IntermediateExpression::IntermediateLambda(lambda)
                if CodeSizeEstimator::estimate_size(&lambda).1 < self.size_limit =>
            {
                let IntermediateLambda {
                    args,
                    statements,
                    ret,
                } = lambda;
                let (statements, internal_continue) = self.inline_statements(statements);
                should_continue |= internal_continue;
                IntermediateLambda {
                    args,
                    statements,
                    ret,
                }
                .into()
            }
            _ => expression,
        };
        statements.push(
            IntermediateAssignment {
                expression,
                location,
            }
            .into(),
        );
        (statements, should_continue)
    }
    fn inline_if_statement(
        &self,
        IntermediateIfStatement {
            condition,
            branches,
        }: IntermediateIfStatement,
    ) -> (Vec<IntermediateStatement>, bool) {
        let branches = (
            self.inline_statements(branches.0),
            self.inline_statements(branches.1),
        );
        (
            vec![IntermediateIfStatement {
                condition,
                branches: (branches.0 .0, branches.1 .0),
            }
            .into()],
            branches.0 .1 || branches.1 .1,
        )
    }
    fn inline_match_statement(
        &self,
        IntermediateMatchStatement { subject, branches }: IntermediateMatchStatement,
    ) -> (Vec<IntermediateStatement>, bool) {
        let (branches, continues): (Vec<_>, Vec<_>) = branches
            .into_iter()
            .map(|IntermediateMatchBranch { target, statements }| {
                let (statements, should_continue) = self.inline_statements(statements);
                (
                    IntermediateMatchBranch { target, statements },
                    should_continue,
                )
            })
            .unzip();
        let should_continue = continues.into_iter().any(identity);
        (
            vec![IntermediateMatchStatement { subject, branches }.into()],
            should_continue,
        )
    }
}

impl From<&Vec<IntermediateStatement>> for Inliner {
    fn from(statements: &Vec<IntermediateStatement>) -> Self {
        let mut inliner = Inliner::new();
        Inliner::collect_fn_defs_from_statements(statements, &mut inliner.fn_defs);
        inliner
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, collections::HashSet, rc::Rc};

    use super::*;
    use lowering::{
        AtomicTypeEnum, Boolean, BuiltInFn, ExpressionEqualityChecker, Id, Integer,
        IntermediateArg, IntermediateAssignment, IntermediateBuiltIn, IntermediateFnCall,
        IntermediateFnType, IntermediateIfStatement, IntermediateMatchBranch,
        IntermediateMatchStatement, IntermediateMemory, IntermediateType, IntermediateUnionType,
        IntermediateValue, Location,
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
    #[test_case(
        {
            let location_1 = Location::new();
            let location_2 = Location::new();
            let location_3 = Location::new();
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
            let lambda_2 = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 13}.into()
            };
            (
                vec![
                    IntermediateMatchStatement {
                        subject: IntermediateArg{
                            location: Location::new(),
                            type_: IntermediateUnionType(vec![None,None,None]).into()
                        }.into(),
                        branches: vec![
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location_1.clone(),
                                        expression: lambda_0.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: location_3.clone(),
                                        expression: lambda_0.clone().into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location_2.clone(),
                                        expression: lambda_1.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: location_3.clone(),
                                        expression: lambda_1.clone().into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location_2.clone(),
                                        expression: lambda_2.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: location_3.clone(),
                                        expression: lambda_2.clone().into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into(),
                ],
                FnDefs::from([
                    (location_1, lambda_0.into())
                ])
            )
        };
        "match statement"
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
                    IntermediateAssignment {
                        location: location_0.clone(),
                        expression: lambda_0.clone().into()
                    }.into(),
                    IntermediateMatchStatement {
                        subject: IntermediateArg{
                            location: Location::new(),
                            type_: IntermediateUnionType(vec![None]).into()
                        }.into(),
                        branches: vec![
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location_1.clone(),
                                        expression: lambda_1.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: Location::new(),
                                        expression: IntermediateValue::from(
                                            IntermediateMemory {
                                                location: location_1.clone(),
                                                type_: IntermediateFnType(
                                                    Vec::new(),
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ).into(),
                                            }
                                        ).into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into(),
                ],
                FnDefs::from([
                    (location_0, lambda_0.into()),
                    (location_1, lambda_1.into()),
                ])
            )
        };
        "match statement with pre-definition"
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
                    IntermediateMatchStatement {
                        subject: IntermediateArg{
                            location: Location::new(),
                            type_: IntermediateUnionType(vec![None,None,None]).into()
                        }.into(),
                        branches: vec![
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location_0.clone(),
                                        expression: lambda_0.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: location_1.clone(),
                                        expression: lambda_1.clone().into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into(),
                ],
                FnDefs::from([
                    (location_0, lambda_0.into())
                ])
            )
        };
        "match statement single branch"
    )]
    #[test_case(
        {
            let internal_location = Location::new();
            let external_location = Location::new();
            let ret_loc = Location::new();
            let internal_lambda = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            };
            let external_lambda = IntermediateLambda {
                args: Vec::new(),
                statements: vec![
                    IntermediateAssignment {
                        location: internal_location.clone(),
                        expression: internal_lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: ret_loc.clone(),
                        expression: IntermediateFnCall{
                            fn_: IntermediateMemory {
                                location: internal_location.clone(),
                                type_: IntermediateFnType(
                                    Vec::new(),
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            }.into(),
                            args: Vec::new()
                        }.into()
                    }.into(),
                ],
                ret: IntermediateMemory {
                    location: ret_loc,
                    type_: AtomicTypeEnum::INT.into()
                }.into()
            };
            (
                vec![
                    IntermediateAssignment {
                        location: external_location.clone(),
                        expression: external_lambda.clone().into()
                    }.into(),
                ],
                FnDefs::from([
                    (internal_location, internal_lambda.into()),
                    (external_location, external_lambda.into()),
                ])
            )
        };
        "nested lambda defs"
    )]
    fn test_collect_fn_defs(statements_fns: (Vec<IntermediateStatement>, FnDefs)) {
        let (statements, expected_fn_defs) = statements_fns;
        let mut fn_defs = FnDefs::new();
        Inliner::collect_fn_defs_from_statements(&statements, &mut fn_defs);
        assert_eq!(fn_defs, expected_fn_defs)
    }

    #[test_case(
        (
            IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 11}.into()
            },
            Vec::new(),
            (
                Vec::new(),
                Integer{value: 11}.into(),
            )
        );
        "trivial fn"
    )]
    #[test_case(
        {
            let arg = IntermediateArg{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let value = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            (
                IntermediateLambda {
                    args: vec![arg.clone()],
                    statements: Vec::new(),
                    ret: arg.clone().into()
                },
                vec![Integer{value: 22}.into()],
                (
                    vec![
                        IntermediateAssignment{
                            location: value.location.clone(),
                            expression: IntermediateValue::from(Integer{value: 22}).into()
                        }.into()
                    ],
                    value.clone().into()
                )
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    location: Location::new(),
                },
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    location: Location::new(),
                },
            ];
            let mem = args.iter().map(|arg| IntermediateMemory {
                location: Location::new(),
                type_: arg.type_.clone()
            }).collect_vec();
            let ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            (
                IntermediateLambda {
                    args: args.clone(),
                    statements: vec![
                        IntermediateAssignment {
                            expression: IntermediateFnCall {
                                fn_: IntermediateBuiltIn::from(BuiltInFn(
                                    Id::from("+"),
                                    IntermediateFnType(
                                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::INT.into()),
                                    )
                                )).into(),
                                args: args.clone().into_iter().map(|arg| arg.into()).collect_vec(),
                            }.into(),
                            location: ret.location.clone()
                        }.into()
                    ],
                    ret: ret.clone().into()
                },
                vec![Integer{value: 11}.into(), Integer{value: -11}.into()],
                (
                    vec![
                        IntermediateAssignment {
                            expression: IntermediateValue::from(Integer{value: 11}).into(),
                            location: mem[0].location.clone()
                        }.into(),
                        IntermediateAssignment {
                            expression: IntermediateValue::from(Integer{value: -11}).into(),
                            location: mem[1].location.clone()
                        }.into(),
                        IntermediateAssignment {
                            expression: IntermediateFnCall {
                                fn_: IntermediateBuiltIn::from(BuiltInFn(
                                    Id::from("+"),
                                    IntermediateFnType(
                                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::INT.into()),
                                    )
                                )).into(),
                                args: mem.clone().into_iter().map(|mem| mem.into()).collect_vec(),
                            }.into(),
                            location: ret.location.clone()
                        }.into()
                    ],
                    ret.clone().into()
                )
            )
        };
        "plus fn"
    )]
    fn test_inline_fn(
        lambda_args_expected: (
            IntermediateLambda,
            Vec<IntermediateValue>,
            (Vec<IntermediateStatement>, IntermediateValue),
        ),
    ) {
        let (lambda, args, expected) = lambda_args_expected;
        let inliner = Inliner::new();
        let mut fn_targets = IntermediateStatement::all_targets(&lambda.statements);
        fn_targets.extend(lambda.args.iter().map(|arg| arg.location.clone()));
        let result = inliner.inline(lambda, args);
        let targets = IntermediateStatement::all_targets(&result.0);

        dbg!(&expected, &result);
        ExpressionEqualityChecker::assert_equal(
            &IntermediateLambda {
                args: Vec::new(),
                statements: result.0,
                ret: result.1,
            }
            .into(),
            &IntermediateLambda {
                args: Vec::new(),
                statements: expected.0,
                ret: expected.1,
            }
            .into(),
        );
        assert!(HashSet::<Location>::from_iter(fn_targets)
            .intersection(&HashSet::from_iter(targets))
            .collect_vec()
            .is_empty())
    }

    #[test]
    fn test_fn_refresh() {
        let id_arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let id = IntermediateLambda {
            args: vec![id_arg.clone()],
            statements: Vec::new(),
            ret: id_arg.clone().into(),
        };
        let id_fn = IntermediateMemory {
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
            location: Location::new(),
        };

        let idea_arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let idea_ret = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let idea = IntermediateLambda {
            args: vec![idea_arg.clone()],
            statements: vec![
                IntermediateAssignment {
                    location: id_fn.location.clone(),
                    expression: id.clone().into(),
                }
                .into(),
                IntermediateAssignment {
                    location: idea_ret.location.clone(),
                    expression: IntermediateFnCall {
                        fn_: id_fn.clone().into(),
                        args: vec![idea_arg.clone().into()],
                    }
                    .into(),
                }
                .into(),
            ],
            ret: idea_ret.clone().into(),
        };

        let inliner = Inliner::new();
        let result = inliner.inline(idea, vec![Integer { value: 0 }.into()]);
        let expected = (
            vec![
                IntermediateAssignment {
                    location: idea_arg.location.clone(),
                    expression: IntermediateValue::from(Integer { value: 0 }).into(),
                }
                .into(),
                IntermediateAssignment {
                    location: id_fn.location.clone(),
                    expression: id.clone().into(),
                }
                .into(),
                IntermediateAssignment {
                    location: idea_ret.location.clone(),
                    expression: IntermediateFnCall {
                        fn_: id_fn.clone().into(),
                        args: vec![IntermediateMemory {
                            type_: idea_arg.type_.clone(),
                            location: idea_arg.location.clone(),
                        }
                        .into()],
                    }
                    .into(),
                }
                .into(),
            ],
            idea_ret.clone().into(),
        );

        dbg!(&expected, &result);
        ExpressionEqualityChecker::assert_equal(
            &IntermediateLambda {
                args: Vec::new(),
                statements: result.0.clone(),
                ret: result.1,
            }
            .into(),
            &IntermediateLambda {
                args: Vec::new(),
                statements: expected.0,
                ret: expected.1,
            }
            .into(),
        );

        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IntermediateLambda(IntermediateLambda {
                    args,
                    statements: _,
                    ret: _,
                }),
            location: _,
        }) = &result.0[1]
        else {
            panic!()
        };
        assert_ne!(args, &vec![id_arg]);
    }

    #[test_case(
        {
            let fn_ = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let ret_location = Location::new();
            let lambda = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Integer{value: 1}.into()
            };
            (
                vec![
                    IntermediateAssignment {
                        location: fn_.location.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: ret_location.clone(),
                        expression: IntermediateFnCall{
                            fn_: fn_.clone().into(),
                            args: Vec::new()
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        location: fn_.location.clone(),
                        expression: lambda.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression: IntermediateValue::from(Integer{value: 1}).into()
                    }.into(),
                ]
            )
        },
        true;
        "trivial fn"
    )]
    #[test_case(
        {
            let fn_ = IntermediateMemory{
                location: Location::new(),
                type_: IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            };
            let ret_location = Location::new();
            let op = IntermediateValue::from(BuiltInFn(
                Id::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            (
                vec![
                    IntermediateAssignment {
                        location: fn_.location.clone(),
                        expression: op.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: ret_location.clone(),
                        expression: IntermediateFnCall{
                            fn_: fn_.clone().into(),
                            args: vec![Integer{value: 3}.into()]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment {
                        location: fn_.location.clone(),
                        expression: op.clone().into()
                    }.into(),
                    IntermediateAssignment {
                        location: ret_location.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone(),
                            args: vec![Integer{value: 3}.into()]
                        }.into()
                    }.into(),
                ]
            )
        },
        false;
        "built-in fn"
    )]
    #[test_case(
        {
            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                statements: Vec::new(),
                ret: id_arg.clone().into(),
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                location: Location::new(),
            };

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea = IntermediateLambda {
                args: vec![idea_arg.clone()],
                statements: vec![
                    IntermediateAssignment {
                        location: id_fn.location.clone(),
                        expression: id.clone().into(),
                    }
                    .into(),
                    IntermediateAssignment {
                        location: idea_ret.location.clone(),
                        expression: IntermediateFnCall {
                            fn_: id_fn.clone().into(),
                            args: vec![idea_arg.clone().into()],
                        }
                        .into(),
                    }
                    .into(),
                ],
                ret: idea_ret.clone().into(),
            };
            let idea_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                location: Location::new(),
            };
            let ret = Location::new();

            let inner_arg = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let outer_res = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let outer_arg = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let fresh_id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            (
                vec![
                    IntermediateAssignment{
                        location: idea_fn.location.clone(),
                        expression: idea.clone().into()
                    }.into(),
                    IntermediateAssignment{
                        location: ret.clone(),
                        expression: IntermediateFnCall{
                            fn_: idea_fn.clone().into(),
                            args: vec![Integer{value: 5}.into()]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        location: idea_fn.location.clone(),
                        expression: IntermediateLambda {
                            args: vec![idea_arg.clone()],
                            ret: idea_ret.clone().into(),
                            statements: vec![
                                IntermediateAssignment {
                                    location: Location::new(),
                                    expression: id.clone().into(),
                                }.into(),
                                IntermediateAssignment{
                                    location: inner_arg.location.clone(),
                                    expression: IntermediateValue::from(
                                        idea_arg.clone()
                                    ).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: idea_ret.location.clone(),
                                    expression: IntermediateValue::from(
                                        inner_arg.clone()
                                    ).into()
                                }.into(),
                            ]
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        location: outer_arg.location.clone(),
                        expression: IntermediateValue::from(
                            Integer{value: 5}
                        ).into()
                    }.into(),
                    IntermediateAssignment{
                        location: id_fn.location.clone(),
                        expression: IntermediateLambda {
                            args: vec![fresh_id_arg.clone()],
                            statements: Vec::new(),
                            ret: fresh_id_arg.clone().into(),
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        location: outer_res.location.clone(),
                        expression: IntermediateFnCall{
                            fn_: id_fn.clone().into(),
                            args: vec![outer_arg.clone().into()]
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        location: ret.clone(),
                        expression: IntermediateValue::from(
                            outer_res
                        ).into()
                    }.into(),
                ],
            )
        },
        true;
        "nested fn"
    )]
    #[test_case(
        {
            let inc = IntermediateValue::from(BuiltInFn(
                Id::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            let dec = IntermediateValue::from(BuiltInFn(
                Id::from("--"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            let op = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                location: Location::new(),
            };

            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                statements: Vec::new(),
                ret: id_arg.clone().into(),
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(vec![AtomicTypeEnum::INT.into()], Box::new(AtomicTypeEnum::INT.into())).into(),
                location: Location::new(),
            };
            let extra = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };

            let ret_location = Location::new();
            let condition = IntermediateArg{
                type_: AtomicTypeEnum::BOOL.into(),
                location: Location::new()
            };
            (
                vec![
                    IntermediateIfStatement {
                        condition: condition.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment {
                                    location: id_fn.location.clone(),
                                    expression: id.clone().into()
                                }.into(),
                                IntermediateAssignment {
                                    location: Location::new(),
                                    expression: IntermediateFnCall{
                                        fn_: id_fn.clone().into(),
                                        args: vec![
                                            IntermediateValue::from(Integer{value: -7}).into()
                                        ]
                                    }.into()
                                }.into(),
                                IntermediateAssignment {
                                    location: op.location.clone(),
                                    expression: inc.clone().into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment {
                                    location: op.location.clone(),
                                    expression: dec.clone().into()
                                }.into(),
                            ],
                        )
                    }.into(),
                    IntermediateAssignment {
                        location: ret_location.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone().into(),
                            args: vec![
                                IntermediateValue::from(Integer{value: -8}).into()
                            ]
                        }.into()
                    }.into(),
                ],
                vec![
                    IntermediateIfStatement {
                        condition: condition.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment {
                                    location: Location::new(),
                                    expression: id.clone().into(),
                                }.into(),
                                IntermediateAssignment{
                                    location: extra.location.clone(),
                                    expression: IntermediateValue::from(
                                        Integer{value: -7}
                                    ).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: Location::new(),
                                    expression: IntermediateValue::from(
                                        extra.clone()
                                    ).into()
                                }.into(),
                                IntermediateAssignment {
                                    location: op.location.clone(),
                                    expression: inc.clone().into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment {
                                    location: op.location.clone(),
                                    expression: dec.clone().into()
                                }.into(),
                            ],
                        )
                    }.into(),
                    IntermediateAssignment {
                        location: ret_location.clone(),
                        expression: IntermediateFnCall{
                            fn_: op.clone().into(),
                            args: vec![
                                IntermediateValue::from(Integer{value: -8}).into()
                            ]
                        }.into()
                    }.into(),               ]
            )
        },
        true;
        "if statement"
    )]
    #[test_case(
        {
            let lambda = IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: Boolean{value: false}.into(),
            };
            let memory = IntermediateMemory {
                type_: IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into())
                ).into(),
                location: Location::new(),
            };

            let subject = IntermediateArg{
                type_: IntermediateUnionType(vec![None]).into(),
                location: Location::new()
            };
            (
                vec![
                    IntermediateMatchStatement {
                        subject: subject.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: memory.location.clone(),
                                        expression: lambda.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: Location::new(),
                                        expression: IntermediateFnCall{
                                            fn_: memory.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into(),
                                ]
                            },
                        ],
                    }.into(),
                ],
                vec![
                    IntermediateMatchStatement {
                        subject: subject.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch {
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: memory.location.clone(),
                                        expression: lambda.clone().into()
                                    }.into(),
                                    IntermediateAssignment {
                                        location: Location::new(),
                                        expression: IntermediateValue::from(
                                            Boolean{value: false}
                                        ).into()
                                    }.into(),
                                ]
                            },
                        ],
                    }.into(),
                ]
            )
        },
        true;
        "match statement"
    )]
    fn test_inlining(
        statements_expected: (Vec<IntermediateStatement>, Vec<IntermediateStatement>),
        expect_continue: bool,
    ) {
        let (statements, expected) = statements_expected;
        let lambda = IntermediateLambda {
            args: Vec::new(),
            ret: Integer { value: 0 }.into(),
            statements,
        };
        let (optimized, should_continue) = Inliner::inline_iteration(lambda, None);
        assert_eq!(expect_continue, should_continue);

        let expected = IntermediateLambda {
            args: Vec::new(),
            ret: Integer { value: 0 }.into(),
            statements: expected,
        };
        dbg!(&expected, &optimized);
        ExpressionEqualityChecker::assert_equal(&optimized.into(), &expected.into())
    }

    #[test]
    fn test_main_inlining() {
        let premain = IntermediateMemory {
            location: Location::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let call = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let simplified = IntermediateLambda {
            args: Vec::new(),
            ret: Integer { value: 0 }.into(),
            statements: Vec::new(),
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            statements: vec![
                IntermediateAssignment {
                    expression: simplified.clone().into(),
                    location: premain.location.clone(),
                }
                .into(),
                IntermediateAssignment {
                    location: call.location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: Vec::new(),
                    }
                    .into(),
                }
                .into(),
            ],
            ret: call.clone().into(),
        };
        let types = vec![Rc::new(RefCell::new(
            IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()), None]).into(),
        ))];
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main,
                types: types.clone(),
            },
            None,
        );
        dbg!(&simplified, &optimized.main);
        ExpressionEqualityChecker::assert_equal(&optimized.main.into(), &simplified.into());
        assert_eq!(types, optimized.types)
    }

    #[test]
    fn test_size_limited_inlining() {
        let premain = IntermediateMemory {
            location: Location::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let call = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let simplified = IntermediateLambda {
            args: Vec::new(),
            ret: Integer { value: 0 }.into(),
            statements: Vec::new(),
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            statements: vec![
                IntermediateAssignment {
                    expression: simplified.clone().into(),
                    location: premain.location.clone(),
                }
                .into(),
                IntermediateAssignment {
                    location: call.location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: Vec::new(),
                    }
                    .into(),
                }
                .into(),
            ],
            ret: call.clone().into(),
        };
        let types = vec![Rc::new(RefCell::new(
            IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()), None]).into(),
        ))];
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: types.clone(),
            },
            Some(1),
        );
        dbg!(&main, &optimized.main);
        ExpressionEqualityChecker::assert_equal(&optimized.main.into(), &main.into());
        assert_eq!(types, optimized.types)
    }

    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateLambda {
                statements: vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                IntermediateAssignment{
                                    location: foo_call.location.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: foo.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into()
                            ],
                            ret: foo_call.clone().into()
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        location: main_call.location.clone(),
                        expression: IntermediateFnCall{
                            fn_: foo.clone().into(),
                            args: Vec::new()
                        }.into()
                    }.into()
                ],
                args: Vec::new(),
                ret: main_call.clone().into(),
            }
        };
        "self recursive fn"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateLambda {
                statements: vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                IntermediateAssignment{
                                    location: bar_call.location.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: bar.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into()
                            ],
                            ret: bar_call.clone().into()
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                IntermediateAssignment{
                                    location: foo_call.location.clone(),
                                    expression: IntermediateFnCall{
                                        fn_: foo.clone().into(),
                                        args: Vec::new()
                                    }.into()
                                }.into()
                            ],
                            ret: foo_call.clone().into()
                        }.into(),
                        location: bar.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        location: main_call.location.clone(),
                        expression: IntermediateFnCall{
                            fn_: foo.clone().into(),
                            args: Vec::new()
                        }.into()
                    }.into()
                ],
                args: Vec::new(),
                ret: main_call.clone().into(),
            }
        };
        "mutually recursive fns"
    )]
    fn test_iterative_inlining(lambda: IntermediateLambda) {
        let mut program = IntermediateProgram {
            main: lambda,
            types: Vec::new(),
        };
        for _ in 1..5 {
            let size = CodeSizeEstimator::estimate_size(&program.main);
            program = Inliner::inline_up_to_size(program, Some(size.1));
            assert!(program.main.find_open_vars().is_empty());
        }
    }

    #[test]
    fn test_recursive_inlining() {
        let premain = IntermediateMemory {
            location: Location::new(),
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
        };
        let call = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };

        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let ret = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let calls = [
            IntermediateMemory {
                location: Location::new(),
                type_: AtomicTypeEnum::INT.into(),
            },
            IntermediateMemory {
                location: Location::new(),
                type_: AtomicTypeEnum::INT.into(),
            },
        ];
        let recursive = IntermediateLambda {
            args: vec![arg.clone()],
            ret: ret.clone().into(),
            statements: vec![
                IntermediateAssignment {
                    location: calls[0].location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: vec![arg.clone().into()],
                    }
                    .into(),
                }
                .into(),
                IntermediateAssignment {
                    location: calls[1].location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: vec![arg.clone().into()],
                    }
                    .into(),
                }
                .into(),
                IntermediateAssignment {
                    location: ret.location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: BuiltInFn(
                            Id::from("+"),
                            IntermediateFnType(
                                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into()),
                            )
                            .into(),
                        )
                        .into(),
                        args: vec![calls[0].clone().into(), calls[1].clone().into()],
                    }
                    .into(),
                }
                .into(),
            ],
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            statements: vec![
                IntermediateAssignment {
                    expression: recursive.clone().into(),
                    location: premain.location.clone(),
                }
                .into(),
                IntermediateAssignment {
                    location: call.location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: vec![Integer { value: 10 }.into()],
                    }
                    .into(),
                }
                .into(),
            ],
            ret: call.clone().into(),
        };
        let current_size = CodeSizeEstimator::estimate_size(&recursive).1;
        let optimized = Inliner::inline_up_to_size(
            IntermediateProgram {
                main,
                types: Vec::new(),
            },
            Some(current_size * 10),
        );
        dbg!(&optimized);
        let optimized_size = CodeSizeEstimator::estimate_size(&optimized.main).1;
        assert!(optimized_size > current_size * 2);
        assert!(optimized_size < current_size * 40);
    }

    #[test]
    fn test_self_recursive_inlining() {
        let premain = IntermediateMemory {
            location: Location::new(),
            type_: IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
        };
        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let call = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };

        let ret = IntermediateMemory {
            location: Location::new(),
            type_: AtomicTypeEnum::INT.into(),
        };
        let recursive = IntermediateLambda {
            args: vec![arg.clone()],
            ret: ret.clone().into(),
            statements: vec![IntermediateAssignment {
                location: ret.location.clone().into(),
                expression: IntermediateFnCall {
                    fn_: premain.clone().into(),
                    args: vec![arg.clone().into()],
                }
                .into(),
            }
            .into()],
        };
        let main = IntermediateLambda {
            args: Vec::new(),
            statements: vec![
                IntermediateAssignment {
                    expression: recursive.clone().into(),
                    location: premain.location.clone(),
                }
                .into(),
                IntermediateAssignment {
                    location: call.location.clone().into(),
                    expression: IntermediateFnCall {
                        fn_: premain.clone().into(),
                        args: vec![Integer { value: -10 }.into()],
                    }
                    .into(),
                }
                .into(),
            ],
            ret: call.clone().into(),
        };
        let current_size = CodeSizeEstimator::estimate_size(&recursive).1;
        Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: Vec::new(),
            },
            Some(current_size * 10),
        );

        Inliner::inline_up_to_size(
            IntermediateProgram {
                main: main.clone(),
                types: Vec::new(),
            },
            None,
        );
    }
}
