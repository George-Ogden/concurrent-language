use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use lowering::{
    AllocationOptimizer, IntermediateAssignment, IntermediateExpression, IntermediateIfStatement,
    IntermediateLambda, IntermediateMatchBranch, IntermediateMatchStatement, IntermediateMemory,
    IntermediateProgram, IntermediateStatement, IntermediateValue, Location,
};

type HistoricalExpressions = HashMap<IntermediateExpression, Location>;
type Definitions = HashMap<Location, IntermediateExpression>;
type NormalizedLocations = HashMap<Location, Location>;

#[derive(Clone)]
pub struct EquivalentExpressionEliminator {
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
            let statements =
                self.weakly_reorder(statements, &vec![location.clone()], &mut HashSet::new());
            let statements =
                self.strongly_reorder(statements, &vec![location], &mut HashSet::new());
            statements
        } else {
            Vec::new()
        };
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
                    let mut new_expression = self.normalize_expression(expression.clone());
                    if let IntermediateExpression::IntermediateLambda(IntermediateLambda {
                        args: _,
                        ref mut statements,
                        ret: _,
                    }) = new_expression
                    {
                        self.prepare_history(statements);
                    }
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
                    self.normalized_locations
                        .insert(location.clone(), new_location);
                    *expression = value.into();
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
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject: _,
                    branches,
                }) => {
                    let normalized_locations = self.normalized_locations.clone();
                    for branch in branches {
                        self.prepare_history(&mut branch.statements);
                    }
                    self.normalized_locations = normalized_locations;
                }
            }
        }
    }
    fn weakly_reorder(
        &self,
        statements: Vec<IntermediateStatement>,
        locations: &Vec<Location>,
        defined: &mut HashSet<Location>,
    ) -> Vec<IntermediateStatement> {
        let mut new_statements = Vec::new();
        let mut weakly_required_locations = self.weakly_required_locations(&statements);
        weakly_required_locations.extend(locations.clone());
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

        for statement in statements {
            for dependency in statement
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
            {
                self.weakly_process_location(
                    dependency,
                    defined,
                    &weakly_required_locations,
                    &mut definitions,
                    &mut new_statements,
                );
            }
            match statement {
                IntermediateStatement::IntermediateAssignment(assignment) => {
                    self.weakly_process_location(
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
                                    &shared_targets,
                                    &mut true_defined,
                                ),
                                self.weakly_reorder(
                                    branches.1,
                                    &shared_targets,
                                    &mut false_defined,
                                ),
                            ),
                        }
                        .into(),
                    );
                    *defined = true_defined.intersection(&false_defined).cloned().collect();
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject,
                    branches,
                }) => {
                    let mut shared_targets: Option<HashSet<Location>> = None;
                    let mut shared_defined: Option<HashSet<Location>> = None;
                    for branch in &branches {
                        let targets = HashSet::from_iter(IntermediateStatement::all_targets(
                            &branch.statements,
                        ));
                        shared_targets = Some(match shared_targets {
                            None => targets,
                            Some(set) => set.intersection(&targets).cloned().collect(),
                        })
                    }
                    let shared_targets = Vec::from_iter(shared_targets.unwrap_or_default());
                    let branches = branches
                        .into_iter()
                        .map(|IntermediateMatchBranch { target, statements }| {
                            let mut defined = defined.clone();
                            let statements =
                                self.weakly_reorder(statements, &shared_targets, &mut defined);
                            shared_defined = Some(match &shared_defined {
                                None => defined,
                                Some(set) => set.intersection(&defined).cloned().collect(),
                            });
                            IntermediateMatchBranch { target, statements }
                        })
                        .collect_vec();
                    new_statements.push(IntermediateMatchStatement { subject, branches }.into());
                    *defined = shared_defined.unwrap_or(defined.clone());
                }
            }
        }
        return new_statements;
    }
    fn weakly_process_location(
        &self,
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

        let Some(mut expression) = definitions.remove(&location) else {
            panic!("Location not found in definitions.")
        };

        let values = if let IntermediateExpression::IntermediateLambda(ref mut lambda) = expression
        {
            lambda.statements = self.weakly_reorder(
                lambda.statements.clone(),
                &lambda
                    .ret
                    .filter_memory_location()
                    .into_iter()
                    .collect_vec(),
                defined,
            );
            lambda
                .find_open_vars()
                .into_iter()
                .map(|memory| memory.location)
                .collect_vec()
        } else {
            expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect_vec()
        };

        for location in values {
            self.weakly_process_location(
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
    fn weakly_required_locations(
        &self,
        statements: &Vec<IntermediateStatement>,
    ) -> HashSet<Location> {
        HashSet::from_iter(statements.iter().flat_map(|statement| match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                let (location, expression) = if let IntermediateExpression::IntermediateValue(
                    IntermediateValue::IntermediateMemory(IntermediateMemory {
                        type_: _,
                        location: normalized_location,
                    }),
                ) = expression
                {
                    let expression = self.definitions[&normalized_location].clone();
                    (normalized_location.clone(), expression)
                } else {
                    (location.clone(), expression.clone())
                };
                let mut locations = vec![location.clone()];
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
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => {
                let mut required = subject.filter_memory_location().into_iter().collect_vec();
                let mut intersection = None;
                for branch in branches {
                    let new_locations = self.weakly_required_locations(&branch.statements);
                    match intersection {
                        None => intersection = Some(new_locations),
                        Some(shared_locations) => {
                            intersection = Some(
                                shared_locations
                                    .intersection(&new_locations)
                                    .cloned()
                                    .collect(),
                            )
                        }
                    }
                }
                required.extend(intersection.unwrap_or_default());
                required
            }
        }))
    }

    fn strongly_reorder(
        &self,
        statements: Vec<IntermediateStatement>,
        locations: &Vec<Location>,
        defined: &mut HashSet<Location>,
    ) -> Vec<IntermediateStatement> {
        let mut new_statements = Vec::new();
        let strongly_required_locations =
            self.strongly_required_locations(&statements, &HashSet::from_iter(locations.clone()));
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

        for statement in statements {
            let values =
                if let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression: IntermediateExpression::IntermediateLambda(lambda),
                    location: _,
                }) = &statement
                {
                    lambda
                        .find_open_vars()
                        .into_iter()
                        .map(|memory| memory.location)
                        .collect_vec()
                } else {
                    statement
                        .values()
                        .iter()
                        .filter_map(IntermediateValue::filter_memory_location)
                        .collect_vec()
                };
            for dependency in values {
                if strongly_required_locations.contains(&dependency) {
                    self.strongly_process_location(
                        dependency,
                        defined,
                        &strongly_required_locations,
                        &mut definitions,
                        &mut new_statements,
                    );
                }
            }
            match statement {
                IntermediateStatement::IntermediateAssignment(assignment) => {
                    if strongly_required_locations.contains(&assignment.location) {
                        self.strongly_process_location(
                            assignment.location,
                            defined,
                            &strongly_required_locations,
                            &mut definitions,
                            &mut new_statements,
                        );
                    }
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
                                self.strongly_reorder(
                                    branches.0.clone(),
                                    &shared_targets,
                                    &mut true_defined,
                                ),
                                self.strongly_reorder(
                                    branches.1,
                                    &shared_targets,
                                    &mut false_defined,
                                ),
                            ),
                        }
                        .into(),
                    );
                    *defined = true_defined.intersection(&false_defined).cloned().collect();
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject,
                    branches,
                }) => {
                    let mut shared_targets: Option<HashSet<Location>> = None;
                    let mut shared_defined: Option<HashSet<Location>> = None;
                    for branch in &branches {
                        let targets = HashSet::from_iter(IntermediateStatement::all_targets(
                            &branch.statements,
                        ));
                        shared_targets = Some(match shared_targets {
                            None => targets,
                            Some(set) => set.intersection(&targets).cloned().collect(),
                        })
                    }
                    let shared_targets = Vec::from_iter(shared_targets.unwrap_or_default());
                    let branches = branches
                        .into_iter()
                        .map(|IntermediateMatchBranch { target, statements }| {
                            let mut defined = defined.clone();
                            let statements =
                                self.strongly_reorder(statements, &shared_targets, &mut defined);
                            shared_defined = Some(match &shared_defined {
                                None => defined,
                                Some(set) => set.intersection(&defined).cloned().collect(),
                            });
                            IntermediateMatchBranch { target, statements }
                        })
                        .collect_vec();
                    new_statements.push(IntermediateMatchStatement { subject, branches }.into());
                    *defined = shared_defined.unwrap_or(defined.clone());
                }
            }
        }
        return new_statements;
    }
    fn strongly_process_location(
        &self,
        location: Location,
        defined: &mut HashSet<Location>,
        strongly_required_locations: &HashSet<Location>,
        definitions: &mut HashMap<Location, IntermediateExpression>,
        new_statements: &mut Vec<IntermediateStatement>,
    ) {
        if defined.contains(&location) {
            return;
        }
        defined.insert(location.clone());

        let Some(mut expression) = definitions.remove(&location) else {
            panic!("Location not found in definitions.");
        };

        let values = if let IntermediateExpression::IntermediateLambda(ref mut lambda) = expression
        {
            lambda.statements = self.strongly_reorder(
                lambda.statements.clone(),
                &lambda
                    .ret
                    .filter_memory_location()
                    .into_iter()
                    .collect_vec(),
                defined,
            );
            lambda
                .find_open_vars()
                .into_iter()
                .map(|memory| memory.location)
                .collect_vec()
        } else {
            expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect_vec()
        };

        for location in values {
            self.strongly_process_location(
                location,
                defined,
                strongly_required_locations,
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
    fn strongly_required_locations(
        &self,
        statements: &Vec<IntermediateStatement>,
        initial_locations: &HashSet<Location>,
    ) -> HashSet<Location> {
        let mut strongly_required_locations = HashSet::from_iter(initial_locations.iter().cloned());
        for statement in statements.iter().rev() {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    if strongly_required_locations.contains(location) {
                        strongly_required_locations.extend(
                            expression
                                .values()
                                .iter()
                                .filter_map(IntermediateValue::filter_memory_location)
                                .collect_vec(),
                        );
                    }
                }
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition,
                    branches,
                }) => {
                    strongly_required_locations
                        .extend(condition.filter_memory_location().into_iter());
                    strongly_required_locations.extend(
                        self.strongly_required_locations(&branches.0, &strongly_required_locations)
                            .intersection(&self.strongly_required_locations(
                                &branches.1,
                                &strongly_required_locations,
                            ))
                            .cloned()
                            .collect_vec(),
                    );
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject,
                    branches,
                }) => {
                    strongly_required_locations
                        .extend(subject.filter_memory_location().into_iter().collect_vec());
                    let mut intersection = None;
                    for branch in branches {
                        let new_locations = self.strongly_required_locations(
                            &branch.statements,
                            &strongly_required_locations,
                        );
                        match intersection {
                            None => {
                                intersection = Some(self.strongly_required_locations(
                                    &branch.statements,
                                    &new_locations,
                                ))
                            }
                            Some(shared_locations) => {
                                intersection = Some(
                                    shared_locations
                                        .intersection(&new_locations)
                                        .cloned()
                                        .collect(),
                                )
                            }
                        }
                    }
                    strongly_required_locations.extend(intersection.unwrap_or_default());
                }
            }
        }
        strongly_required_locations
    }

    pub fn eliminate_equivalent_expressions(program: IntermediateProgram) -> IntermediateProgram {
        let IntermediateProgram { main, types } = program;
        let mut optimizer = EquivalentExpressionEliminator::new();
        let lambda = optimizer.eliminate_from_lambda(main);
        let allocation_optimizer = AllocationOptimizer::from_statements(&lambda.statements);
        let IntermediateExpression::IntermediateLambda(main) =
            allocation_optimizer.remove_wasted_allocations_from_expression(lambda.into())
        else {
            panic!("Main function changed form.")
        };
        IntermediateProgram { main, types }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    use lowering::{
        AllocationOptimizer, AtomicTypeEnum, ExpressionEqualityChecker, Integer, IntermediateArg,
        IntermediateAssignment, IntermediateBuiltIn, IntermediateElementAccess, IntermediateFnCall,
        IntermediateFnType, IntermediateIfStatement, IntermediateLambda, IntermediateMatchBranch,
        IntermediateMatchStatement, IntermediateMemory, IntermediateProgram,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateUnionType, IntermediateValue, Location,
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
    #[test_case(
        {
            let w = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let z = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let s = IntermediateArg::from(IntermediateType::from(IntermediateUnionType(vec![None, None, Some(AtomicTypeEnum::INT.into())])));
            (
                vec![
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            Integer{value: 0}.into(),
                                        ]).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            Integer{value: 1}.into(),
                                        ]).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(arg.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            arg.clone().into()
                                        ]).into()
                                    }.into()
                                ],
                            },
                        ]
                    }.into(),
                    IntermediateAssignment{
                        location: y.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        location: y.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateValue::from(y.clone()).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: w.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            Integer{value: 1}.into(),
                                        ]).into()
                                    }.into(),
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateValue::from(w.clone()).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(arg.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: z.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            arg.clone().into()
                                        ]).into()
                                    }.into(),
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateValue::from(z.clone()).into()
                                    }.into()
                                ],
                            },
                        ]
                    }.into(),
                ],
                vec![
                    x.location.clone(),
                    y.location.clone(),
                ]
            )
        };
        "match statement shared value after branch"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(vec![AtomicTypeEnum::INT.into()]).into(),AtomicTypeEnum::INT.into()])));
            let z = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let c = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![
                    IntermediateAssignment{
                        location: y.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: x.location.clone(),
                                    expression: IntermediateValue::from(y.clone()).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: x.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        Integer{value: 1}.into(),
                                    ]).into()
                                }.into()
                            ]
                        )
                    }.into(),
                ],
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: y.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        Integer{value: 0}.into(),
                                    ]).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: x.location.clone(),
                                    expression: IntermediateValue::from(y.clone()).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: z.location.clone(),
                                    expression: IntermediateTupleExpression(vec![
                                        Integer{value: 1}.into(),
                                    ]).into()
                                }.into(),
                                IntermediateAssignment{
                                    location: x.location.clone(),
                                    expression: IntermediateValue::from(z.clone()).into()
                                }.into(),
                            ]
                        )
                    }.into()
                ],
                vec![
                    x.location.clone()
                ]
            )
        };
        "if statement non-shared value available"
    )]
    #[test_case(
        {
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let s = IntermediateArg::from(IntermediateType::from(IntermediateUnionType(vec![None, None])));
            (
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateAssignment{
                        location: z.location.clone(),
                        expression: IntermediateElementAccess{
                            value: x.clone().into(),
                            idx: 0
                        }.into()
                    }.into(),
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: y.location.clone(),
                                        expression: IntermediateValue::from(
                                            z.clone()
                                        ).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: y.location.clone(),
                                        expression: IntermediateValue::from(
                                            Integer{value: 1}
                                        ).into()
                                    }.into()
                                ]
                            }
                        ]
                    }.into(),
                ],
                vec![
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: x.location.clone(),
                                        expression: IntermediateTupleExpression(vec![
                                            Integer{value: 0}.into(),
                                        ]).into()
                                    }.into(),
                                    IntermediateAssignment{
                                        location: z.location.clone(),
                                        expression: IntermediateElementAccess{
                                            value: x.clone().into(),
                                            idx: 0
                                        }.into()
                                    }.into(),
                                    IntermediateAssignment{
                                        location: y.location.clone(),
                                        expression: IntermediateValue::from(
                                            z.clone()
                                        ).into()
                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: y.location.clone(),
                                        expression: IntermediateValue::from(
                                            Integer{value: 1}
                                        ).into()
                                    }.into(),
                                ]
                            }
                        ]
                    }.into(),
                ],
                vec![
                    y.location.clone()
                ]
            )
        };
        "match statement nested non-shared value available"
    )]
    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let lambda = Location::new();
            (
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                ret.clone().into()
                            ],
                            ret: ret.clone().into()
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: Vec::new(),
                            ret: assignment.clone().into()
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    assignment.location.clone(),
                    lambda.clone()
                ]
            )
        };
        "fn body external reassignment"
    )]
    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let lambda = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                assignment.clone().into(),
                                ret.clone().into()
                            ],
                            ret: ret.clone().into()
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            statements: vec![
                                assignment.clone().into(),
                            ],
                            ret: assignment.clone().into()
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    lambda.clone()
                ]
            )
        };
        "fn body internal reassignment"
    )]
    #[test_case(
        {
            let lambda = IntermediateMemory::from(IntermediateType::from(
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )
            ));
            let arg = IntermediateArg::from(IntermediateType::from(
                AtomicTypeEnum::INT
            ));
            let call = IntermediateAssignment{
                location: Location::new(),
                expression: IntermediateFnCall{
                    fn_: lambda.clone().into(),
                    args: vec![arg.clone().into()]
                }.into()
            };
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![arg.clone()],
                            statements: vec![
                                call.clone().into()
                            ],
                            ret: call.clone().into()
                        }.into(),
                        location: lambda.location.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![arg.clone()],
                            statements: vec![
                                call.clone().into()
                            ],
                            ret: call.clone().into()
                        }.into(),
                        location: lambda.location.clone()
                    }.into()
                ],
                vec![
                    lambda.location.clone()
                ]
            )
        };
        "recursive fn"
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
        let optimized_fn =
            allocation_optimizer.remove_wasted_allocations_from_expression(optimized_fn.into());
        assert!(ExpressionEqualityChecker::equal(
            &optimized_fn,
            &expected_fn.into()
        ));
    }

    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                location: Location::new()
            };
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![None, None]).into()))
            ];
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        statements: vec![
                            assignment.clone().into(),
                            ret.clone().into()
                        ],
                        ret: ret.clone().into()
                    },
                    types: types.clone()
                },
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        statements: vec![
                            assignment.clone().into()
                        ],
                        ret: assignment.clone().into()
                    }.into(),
                    types
                }.into()
            )
        };
        "basic program"
    )]
    fn test_eliminate_program_equivalent_expressions(
        program_expected: (IntermediateProgram, IntermediateProgram),
    ) {
        let (program, expected_program) = program_expected;
        let optimized_program =
            EquivalentExpressionEliminator::eliminate_equivalent_expressions(program);
        dbg!(&optimized_program);
        dbg!(&expected_program);
        assert_eq!(optimized_program.types, expected_program.types);
        assert!(ExpressionEqualityChecker::equal(
            &optimized_program.main.into(),
            &expected_program.main.into()
        ))
    }
}
