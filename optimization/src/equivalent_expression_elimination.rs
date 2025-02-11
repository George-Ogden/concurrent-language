use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use lowering::{
    IntermediateAssignment, IntermediateExpression, IntermediateIfStatement, IntermediateMemory,
    IntermediateStatement, IntermediateValue, Location,
};

type HistoricalExpressions = HashMap<IntermediateExpression, Location>;
type NormalizedLocations = HashMap<Location, Location>;

#[derive(Clone)]
struct EquivalentExpressionEliminator {
    historical_expressions: HistoricalExpressions,
    normalized_locations: NormalizedLocations,
}

impl EquivalentExpressionEliminator {
    pub fn new() -> Self {
        Self {
            historical_expressions: HistoricalExpressions::new(),
            normalized_locations: NormalizedLocations::new(),
        }
    }

    fn normalize_expression(
        &self,
        mut expression: IntermediateExpression,
    ) -> IntermediateExpression {
        expression.substitute(&self.normalized_locations);
        expression
    }

    fn eliminate_from_statement(
        &mut self,
        statement: IntermediateStatement,
    ) -> Vec<IntermediateStatement> {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                let expression = self.normalize_expression(expression);
                vec![match self.historical_expressions.get(&expression) {
                    None => {
                        self.historical_expressions
                            .insert(expression.clone(), location.clone());
                        IntermediateAssignment {
                            expression,
                            location,
                        }
                    }
                    Some(updated_location) => {
                        if updated_location == &location {
                            return Vec::new();
                        }
                        self.normalized_locations
                            .insert(location.clone(), updated_location.clone());
                        IntermediateAssignment {
                            location,
                            expression: IntermediateValue::from(IntermediateMemory {
                                location: updated_location.clone(),
                                type_: expression.type_(),
                            })
                            .into(),
                        }
                    }
                }
                .into()]
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                let mut true_eliminator = self.clone();
                let mut false_eliminator = self.clone();
                true_eliminator.eliminate_from_statements(branches.0.clone());
                false_eliminator.eliminate_from_statements(branches.1.clone());
                let true_expressions: HashSet<&IntermediateExpression> =
                    HashSet::from_iter(true_eliminator.historical_expressions.keys());
                let false_expressions: HashSet<&IntermediateExpression> =
                    HashSet::from_iter(false_eliminator.historical_expressions.keys());
                let shared_expressions = true_expressions.intersection(&false_expressions);
                let statements = shared_expressions
                    .map(|&expression| {
                        IntermediateAssignment {
                            location: true_eliminator.historical_expressions[&expression].clone(),
                            expression: expression.clone(),
                        }
                        .into()
                    })
                    .collect_vec();
                let mut statements = self.eliminate_from_statements(statements);
                statements.push(
                    IntermediateIfStatement {
                        condition,
                        branches: (
                            self.clone().eliminate_from_statements(branches.0),
                            self.clone().eliminate_from_statements(branches.1),
                        ),
                    }
                    .into(),
                );
                statements
            }
            IntermediateStatement::IntermediateMatchStatement(intermediate_match_statement) => {
                todo!()
            }
        }
    }
    fn eliminate_from_statements(
        &mut self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        statements
            .into_iter()
            .flat_map(|statement| self.eliminate_from_statement(statement))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use lowering::{
        AllocationOptimizer, AtomicTypeEnum, Integer, IntermediateArg, IntermediateAssignment,
        IntermediateBuiltIn, IntermediateIfStatement, IntermediateMemory,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType, IntermediateValue,
        Location,
    };
    use test_case::test_case;

    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let memory = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
            let assignment_0 = IntermediateAssignment{
                expression: expression.clone().into(),
                location: memory.location.clone()
            };
            let assignment_1 = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            (
                vec![
                    assignment_0.clone().into(),
                    assignment_1.clone().into()
                ],
                vec![
                    assignment_0.clone().into(),
                    IntermediateAssignment{
                        location: assignment_1.location.clone(),
                        expression: IntermediateValue::from(
                            memory.clone()
                        ).into()
                    }.into()
                ]
            )
        };
        "empty tuple assignment"
    )]
    fn test_raw_eliminate(statements: (Vec<IntermediateStatement>, Vec<IntermediateStatement>)) {
        let (original_statements, expected_statements) = statements;
        let mut equivalent_expression_eliminator = EquivalentExpressionEliminator::new();
        let optimized_statements =
            equivalent_expression_eliminator.eliminate_from_statements(original_statements);
        assert_eq!(optimized_statements, expected_statements);
    }

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
                ]
            )
        };
        "repeated nested empty tuple assignment assignment"
    )]
    #[test_case(
        {
            let a = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let b = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
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
                                    expression: IntermediateValue::from(a.clone()).into()
                                }.into(),
                            ]
                        )
                    }.into()
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
                                    location: y.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        x.clone().into(),
                                        Integer{value: 0}.into(),
                                    ]).into()
                                }.into()
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        x.clone().into(),
                                        Integer{value: 1}.into(),
                                    ]).into()
                                }.into(),
                            ]
                        )
                    }.into()
                ]
            )
        };
        "if statement shared value across branch"
    )]
    fn test_eliminate(statements: (Vec<IntermediateStatement>, Vec<IntermediateStatement>)) {
        let (original_statements, expected_statements) = statements;
        let mut equivalent_expression_eliminator = EquivalentExpressionEliminator::new();
        let optimized_statements =
            equivalent_expression_eliminator.eliminate_from_statements(original_statements);
        let allocation_optimizer = AllocationOptimizer::from_statements(&optimized_statements);
        dbg!(&optimized_statements);
        let optimized_statements =
            allocation_optimizer.remove_wasted_allocations_from_statements(optimized_statements);
        assert_eq!(optimized_statements, expected_statements);
    }
}
