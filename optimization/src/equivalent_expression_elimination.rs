use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use itertools::Itertools;
use lowering::{
    IntermediateAssignment, IntermediateExpression, IntermediateIfStatement, IntermediateLambda,
    IntermediateMemory, IntermediateStatement, IntermediateTupleType, IntermediateValue, Location,
};

type HistoricalExpressions = HashMap<IntermediateExpression, Location>;
type Definitions = HashMap<Location, IntermediateExpression>;
type NormalizedLocations = HashMap<Location, Location>;

#[derive(Clone)]
struct EquivalentExpressionEliminator {
    historical_expressions: HistoricalExpressions,
    definitions: Definitions,
    normalized_locations: NormalizedLocations,
}

impl EquivalentExpressionEliminator {
    pub fn new() -> Self {
        Self {
            historical_expressions: HistoricalExpressions::new(),
            normalized_locations: NormalizedLocations::new(),
            definitions: Definitions::new(),
        }
    }

    fn normalize_expression(
        &self,
        mut expression: IntermediateExpression,
    ) -> IntermediateExpression {
        expression.substitute(&self.normalized_locations);
        expression
    }

    fn eliminate_from_lambda(&mut self, lambda: IntermediateLambda) -> IntermediateLambda {
        let IntermediateLambda {
            args,
            mut statements,
            ret,
        } = lambda;
        self.prepare_history(&mut statements);
        let statements = if let Some(location) = IntermediateValue::filter_memory_location(&ret) {
            self.weakly_reorder(statements, vec![location], &mut HashSet::new())
        } else {
            Vec::new()
        };
        // TODO: strongly reorder
        IntermediateLambda {
            args,
            statements,
            ret,
        }
    }
    fn prepare_history(&mut self, statements: &mut Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    let new_expression = self.normalize_expression(expression.clone());
                    let new_location = match self.historical_expressions.get(&new_expression) {
                        None => {
                            let new_location = Location::new();
                            self.historical_expressions
                                .insert(new_expression.clone(), new_location.clone());
                            self.definitions
                                .insert(new_location.clone(), new_expression.clone());
                            new_location
                        }
                        Some(new_location) => new_location.clone(),
                    };
                    let value = IntermediateValue::from(IntermediateMemory {
                        type_: expression.type_(),
                        location: new_location.clone(),
                    });
                    self.definitions
                        .insert(location.clone(), value.clone().into());
                    *expression = value.into();
                    self.normalized_locations
                        .insert(location.clone(), new_location);
                }
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition: _,
                    branches,
                }) => {
                    let normalized_locations = self.normalized_locations.clone();
                    self.prepare_history(&mut branches.0);
                    self.prepare_history(&mut branches.1);
                    self.normalized_locations = normalized_locations;
                }
                IntermediateStatement::IntermediateMatchStatement(_) => {
                    todo!()
                }
            }
        }
    }
    fn weakly_reorder(
        &mut self,
        statements: Vec<IntermediateStatement>,
        locations: Vec<Location>,
        defined: &mut HashSet<Location>,
    ) -> Vec<IntermediateStatement> {
        let mut new_statements = Vec::new();
        let mut weakly_required_locations = self.weakly_required_locations(&statements);
        weakly_required_locations.extend(locations);
        let mut definitions = self.definitions.clone();
        for statement in &statements {
            if let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) = statement
            {
                *definitions.get_mut(&location).unwrap() = expression.clone();
            }
        }

        fn process_location(
            location: Location,
            defined: &mut HashSet<Location>,
            weakly_required_locations: &HashSet<Location>,
            definitions: &mut HashMap<Location, IntermediateExpression>,
            new_statements: &mut Vec<IntermediateStatement>,
        ) {
            if defined.contains(&location) || !weakly_required_locations.contains(&location) {
                return;
            }
            defined.insert(location.clone());

            let Some(expression) = definitions.remove(&location) else {
                panic!("Location not found in definitions.")
            };

            for location in expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
            {
                process_location(
                    location,
                    defined,
                    weakly_required_locations,
                    definitions,
                    new_statements,
                );
            }

            new_statements.push(
                IntermediateAssignment {
                    location,
                    expression,
                }
                .into(),
            );
        }

        for statement in statements {
            for dependency in statement
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
            {
                process_location(
                    dependency,
                    defined,
                    &weakly_required_locations,
                    &mut definitions,
                    &mut new_statements,
                );
            }
            match statement {
                IntermediateStatement::IntermediateAssignment(assignment) => {
                    process_location(
                        assignment.location,
                        defined,
                        &weakly_required_locations,
                        &mut definitions,
                        &mut new_statements,
                    );
                }
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition,
                    branches,
                }) => {
                    let targets = (
                        HashSet::<Location>::from_iter(IntermediateStatement::all_targets(
                            &branches.0,
                        )),
                        HashSet::<Location>::from_iter(IntermediateStatement::all_targets(
                            &branches.1,
                        )),
                    );
                    let shared_targets = targets.0.intersection(&targets.1).cloned().collect_vec();
                    let mut true_defined = defined.clone();
                    let mut false_defined = defined.clone();
                    new_statements.push(
                        IntermediateIfStatement {
                            condition,
                            branches: (
                                self.weakly_reorder(
                                    branches.0.clone(),
                                    shared_targets.clone(),
                                    &mut true_defined,
                                ),
                                self.weakly_reorder(
                                    branches.1,
                                    shared_targets.clone(),
                                    &mut false_defined,
                                ),
                            ),
                        }
                        .into(),
                    );
                    *defined = true_defined.intersection(&false_defined).cloned().collect();
                }
                IntermediateStatement::IntermediateMatchStatement(intermediate_match_statement) => {
                    todo!()
                }
            }
        }
        return new_statements;
    }
    fn weakly_required_locations(
        &self,
        statements: &Vec<IntermediateStatement>,
    ) -> HashSet<Location> {
        HashSet::from_iter(statements.iter().flat_map(|statement| match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                let IntermediateExpression::IntermediateValue(
                    IntermediateValue::IntermediateMemory(IntermediateMemory {
                        type_: _,
                        location: normalized_location,
                    }),
                ) = expression
                else {
                    panic!("Expression not correctly converted.")
                };
                let expression = self.definitions[&normalized_location].clone();
                let mut locations = vec![normalized_location.clone()];
                locations.extend(
                    expression
                        .values()
                        .iter()
                        .filter_map(IntermediateValue::filter_memory_location)
                        .collect_vec(),
                );
                locations
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                let mut required = condition.filter_memory_location().into_iter().collect_vec();
                required.extend(
                    self.weakly_required_locations(&branches.0)
                        .intersection(&self.weakly_required_locations(&branches.1))
                        .cloned()
                        .collect_vec(),
                );
                required
            }
            IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use lowering::{
        AllocationOptimizer, AtomicTypeEnum, ExpressionEqualityChecker, Integer, IntermediateArg,
        IntermediateAssignment, IntermediateBuiltIn, IntermediateIfStatement, IntermediateLambda,
        IntermediateMemory, IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateValue, Location,
    };
    use test_case::test_case;

    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            (
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: expression.clone().into(),
                        location: Location::new()
                    }.into()
                ],
                vec![
                    assignment.clone().into(),
                ],
                vec![
                    assignment.location.clone()
                ]
            )
        };
        "repeated empty tuple assignment"
    )]
    #[test_case(
        {
            let empty_location_0 = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
            let empty_location_1 = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
            let empty_assignment_0 = IntermediateAssignment{
                expression: IntermediateTupleExpression(Vec::new()).into(),
                location: empty_location_0.location.clone()
            };
            let empty_assignment_1 = IntermediateAssignment{
                expression: IntermediateTupleExpression(Vec::new()).into(),
                location: empty_location_1.location.clone()
            };
            let nested_assignment_0 = IntermediateAssignment{
                expression: IntermediateTupleExpression(vec![empty_location_0.clone().into()]).into(),
                location: Location::new()
            };
            let nested_assignment_1 = IntermediateAssignment{
                expression: IntermediateTupleExpression(vec![empty_location_1.clone().into()]).into(),
                location: Location::new()
            };
            (
                vec![
                    empty_assignment_0.clone().into(),
                    empty_assignment_1.clone().into(),
                    nested_assignment_0.clone().into(),
                    nested_assignment_1.clone().into(),
                ],
                vec![
                    empty_assignment_0.clone().into(),
                    nested_assignment_0.clone().into(),
                ],
                vec![
                    nested_assignment_0.location.clone(),
                    nested_assignment_0.location.clone()
                ]
            )
        };
        "repeated nested empty tuple assignment assignment"
    )]
    #[test_case(
        {
            let a = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let b = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let extra = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let zero = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()]);
            let one = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()]);
            let c = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: b.location.clone(),
                                    expression: zero.clone().into()
                                }.into()
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: a.location.clone(),
                                    expression: one.clone().into()
                                }.into(),
                                IntermediateAssignment{
                                    location: b.location.clone(),
                                    expression: one.clone().into()
                                }.into(),
                            ]
                        )
                    }.into()
                ],
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: extra.location.clone(),
                                    expression: zero.clone().into()
                                }.into(),
                                IntermediateAssignment{
                                    location: b.location.clone(),
                                    expression: IntermediateValue::from(extra.clone()).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: a.location.clone(),
                                    expression: one.clone().into()
                                }.into(),
                                IntermediateAssignment{
                                    location: b.location.clone(),
                                    expression: IntermediateValue::from(a.clone()).into()
                                }.into(),
                            ]
                        )
                    }.into()
                ],
                vec![
                    b.location.clone()
                ]
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(vec![AtomicTypeEnum::INT.into()]).into(),AtomicTypeEnum::INT.into()])));
            let z = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let a = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let b = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let eight = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 8})).into()]);
            let c = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: x.location.clone(),
                                    expression: eight.clone().into()
                                }.into(),
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        x.clone().into(),
                                        Integer{value: 0}.into(),
                                    ]).into()
                                }.into()
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: z.location.clone(),
                                    expression: eight.clone().into()
                                }.into(),
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        z.clone().into(),
                                        Integer{value: 1}.into(),
                                    ]).into()
                                }.into(),
                            ]
                        )
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: eight.clone().into()
                    }.into(),
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: a.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        x.clone().into(),
                                        Integer{value: 0}.into(),
                                    ]).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateValue::from(a.clone()).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: b.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        x.clone().into(),
                                        Integer{value: 1}.into(),
                                    ]).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateValue::from(b.clone()).into()
                                }.into(),
                            ]
                        )
                    }.into()
                ],
                vec![
                    y.location.clone()
                ]
            )
        };
        "if statement shared value across branch"
    )]
    fn test_eliminate(
        statements: (
            Vec<IntermediateStatement>,
            Vec<IntermediateStatement>,
            Vec<Location>,
        ),
    ) {
        let (mut original_statements, mut expected_statements, required) = statements;
        let original_location =
            IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
        let expected_location =
            IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
        let return_expression = IntermediateTupleExpression(
            required
                .into_iter()
                .map(|location| {
                    IntermediateMemory {
                        type_: IntermediateTupleType(Vec::new()).into(),
                        location,
                    }
                    .into()
                })
                .collect(),
        );
        original_statements.push(
            IntermediateAssignment {
                location: original_location.location.clone(),
                expression: return_expression.clone().into(),
            }
            .into(),
        );
        expected_statements.push(
            IntermediateAssignment {
                location: expected_location.location.clone(),
                expression: return_expression.clone().into(),
            }
            .into(),
        );
        let expected_fn = IntermediateLambda {
            args: Vec::new(),
            statements: expected_statements,
            ret: expected_location.clone().into(),
        };
        let mut equivalent_expression_eliminator = EquivalentExpressionEliminator::new();
        let optimized_fn =
            equivalent_expression_eliminator.eliminate_from_lambda(IntermediateLambda {
                args: Vec::new(),
                statements: original_statements,
                ret: original_location.clone().into(),
            });
        let allocation_optimizer = AllocationOptimizer::from_statements(&optimized_fn.statements);
        dbg!(&optimized_fn);
        let optimized_fn =
            allocation_optimizer.remove_wasted_allocations_from_expression(optimized_fn.into());
        dbg!(&optimized_fn, &expected_fn);
        assert!(ExpressionEqualityChecker::equal(
            &optimized_fn,
            &expected_fn.into()
        ));
    }
}
