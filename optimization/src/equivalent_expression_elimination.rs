use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use lowering::{
    AllocationOptimizer, IBlock, ILambda, IntermediateAssignment, IntermediateExpression,
    IntermediateMemory, IntermediateProgram, IntermediateStatement, IntermediateValue, Location,
};

// use crate::refresher::Refresher;

type HistoricalExpressions = HashMap<IntermediateExpression, Location>;
type Definitions = HashMap<Location, IntermediateExpression>;
type NormalizedLocations = HashMap<Location, Location>;
type OpenVars = HashMap<IBlock, Vec<IntermediateValue>>;

#[derive(Clone)]
pub struct EquivalentExpressionEliminator {
    historical_expressions: HistoricalExpressions,
    definitions: Definitions,
    normalized_locations: NormalizedLocations,
    open_vars: OpenVars,
}

impl EquivalentExpressionEliminator {
    pub fn new() -> Self {
        Self {
            historical_expressions: HistoricalExpressions::new(),
            normalized_locations: NormalizedLocations::new(),
            definitions: Definitions::new(),
            open_vars: OpenVars::new(),
        }
    }

    fn normalize_expression(
        &self,
        mut expression: IntermediateExpression,
    ) -> IntermediateExpression {
        expression.substitute(&self.normalized_locations);
        expression
    }

    fn eliminate_from_lambda(&mut self, lambda: ILambda) -> ILambda {
        let ILambda { args, mut block } = lambda;
        self.prepare_history(&mut block);
        let block = self.weakly_reorder(block, &mut HashSet::new());
        self.refresh_history(&block);
        let block = self.strongly_reorder(block, &mut HashSet::new());
        let mut lambda = ILambda { args, block };
        // Refresher::refresh(&mut lambda);
        lambda
    }
    fn prepare_history(&mut self, block: &mut IBlock) {
        for statement in &mut block.statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    let mut new_expression = self.normalize_expression(expression.clone());
                    match new_expression {
                        IntermediateExpression::ILambda(ref mut lambda) => {
                            let open_vars = lambda.find_open_vars();
                            self.prepare_history(&mut lambda.block);
                            self.open_vars
                                .insert(lambda.block.clone(), open_vars.clone());
                        }
                        IntermediateExpression::IIf(ref mut if_) => {
                            for block in [&mut if_.branches.0, &mut if_.branches.1] {
                                let open_vars = block.values();
                                self.prepare_history(block);
                                self.open_vars.insert(block.clone(), open_vars.clone());
                            }
                        }
                        IntermediateExpression::IMatch(ref mut match_) => {
                            for branch in &mut match_.branches {
                                self.prepare_history(&mut branch.block);
                            }
                        }
                        _ => {}
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
            }
        }
    }
    fn refresh_history(&mut self, block: &IBlock) {
        for statement in &block.statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    match &expression {
                        IntermediateExpression::ILambda(ref lambda) => {
                            self.refresh_history(&lambda.block);
                        }
                        IntermediateExpression::IIf(ref if_) => {
                            for block in [&if_.branches.0, &if_.branches.1] {
                                self.refresh_history(&block);
                            }
                        }
                        IntermediateExpression::IMatch(ref match_) => {
                            for branch in &match_.branches {
                                self.refresh_history(&branch.block);
                            }
                        }
                        _ => {}
                    }
                    self.definitions
                        .insert(location.clone(), expression.clone());
                }
            }
        }
    }
    fn weakly_reorder(&self, block: IBlock, defined: &mut HashSet<Location>) -> IBlock {
        let mut new_statements = Vec::new();
        let weakly_required_locations = self.weak_block_locations(&block);

        let IBlock { statements, ret } = block;

        for statement in statements {
            let values = statement.values();
            for value in values
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
            {
                self.weakly_process_location(
                    value,
                    defined,
                    &weakly_required_locations,
                    &mut new_statements,
                );
            }
            match statement {
                IntermediateStatement::IntermediateAssignment(assignment) => {
                    self.weakly_process_location(
                        assignment.location,
                        defined,
                        &weakly_required_locations,
                        &mut new_statements,
                    );
                }
            }
        }
        IBlock {
            statements: new_statements,
            ret,
        }
    }
    fn weakly_process_location(
        &self,
        location: Location,
        defined: &mut HashSet<Location>,
        weakly_required_locations: &HashSet<Location>,
        new_statements: &mut Vec<IntermediateStatement>,
    ) {
        if defined.contains(&location) || !weakly_required_locations.contains(&location) {
            return;
        }
        defined.insert(location.clone());

        let Some(mut expression) = self.definitions.get(&location).cloned() else {
            return;
        };

        for location in self.very_weak_expression_locations(&expression) {
            self.weakly_process_location(
                location,
                defined,
                weakly_required_locations,
                new_statements,
            );
        }
        match &mut expression {
            IntermediateExpression::ILambda(lambda) => {
                lambda.block = self.weakly_reorder(lambda.block.clone(), defined);
            }
            IntermediateExpression::IIf(if_) => {
                if_.branches.0 = self.weakly_reorder(if_.branches.0.clone(), defined);
                if_.branches.1 = self.weakly_reorder(if_.branches.1.clone(), defined);
            }
            IntermediateExpression::IMatch(match_) => {
                for branch in &mut match_.branches {
                    branch.block = self.weakly_reorder(branch.block.clone(), defined);
                }
            }
            _ => {}
        }

        new_statements.push(
            IntermediateAssignment {
                location,
                expression,
            }
            .into(),
        );
    }
    fn weak_block_locations(&self, block: &IBlock) -> HashSet<Location> {
        let mut locations =
            HashSet::from_iter(
                block
                    .statements
                    .iter()
                    .flat_map(|statement| match statement {
                        IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                            expression,
                            location,
                        }) => {
                            let (location, expression) =
                                if let IntermediateExpression::IntermediateValue(
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
                            locations
                                .extend(self.weak_expression_locations(&expression).into_iter());
                            locations
                        }
                    }),
            );
        locations.extend(block.ret.filter_memory_location().into_iter());
        locations
    }
    pub fn weak_expression_locations(
        &self,
        expression: &IntermediateExpression,
    ) -> HashSet<Location> {
        let merge = |a, b| {
            HashSet::intersection(&a, &b)
                .cloned()
                .collect::<HashSet<_>>()
        };
        match &expression {
            IntermediateExpression::ILambda(lambda) => self.open_vars[&lambda.block]
                .clone()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect(),
            IntermediateExpression::IIf(if_) => {
                let mut required = merge(
                    self.weak_block_locations(&if_.branches.0),
                    self.weak_block_locations(&if_.branches.1),
                );
                required.extend(if_.condition.filter_memory_location().into_iter());
                required
            }
            IntermediateExpression::IMatch(match_) => {
                let mut required = None;
                if match_.branches.len() > 1 {
                    for branch in &match_.branches {
                        let extra = self.weak_block_locations(&branch.block);
                        required = match required {
                            Some(current) => Some(merge(extra, current)),
                            None => Some(extra),
                        }
                    }
                }
                required.unwrap_or_default()
            }
            expression => expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect(),
        }
    }
    pub fn very_weak_expression_locations(
        &self,
        expression: &IntermediateExpression,
    ) -> HashSet<Location> {
        match &expression {
            IntermediateExpression::ILambda(lambda) => self.open_vars[&lambda.block]
                .clone()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect(),
            IntermediateExpression::IIf(if_) => {
                let mut required = self.very_weak_block_locations(&if_.branches.0);
                required.extend(self.very_weak_block_locations(&if_.branches.1));
                required.extend(if_.condition.filter_memory_location().into_iter());
                required
            }
            IntermediateExpression::IMatch(match_) => match_
                .branches
                .iter()
                .flat_map(|branch| self.very_weak_block_locations(&branch.block))
                .collect(),
            expression => expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_location)
                .collect(),
        }
    }
    fn very_weak_block_locations(&self, block: &IBlock) -> HashSet<Location> {
        let mut locations =
            HashSet::from_iter(
                block
                    .statements
                    .iter()
                    .flat_map(|statement| match statement {
                        IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                            expression,
                            location,
                        }) => {
                            let (location, expression) =
                                if let IntermediateExpression::IntermediateValue(
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
                                self.very_weak_expression_locations(&expression).into_iter(),
                            );
                            locations
                        }
                    }),
            );
        locations.extend(block.ret.filter_memory_location().into_iter());
        locations
    }

    fn strongly_reorder(&self, block: IBlock, defined: &mut HashSet<Location>) -> IBlock {
        let mut new_statements = Vec::new();
        let ret = if let IntermediateValue::IntermediateMemory(memory) = &block.ret {
            let strongly_required_locations = self.strong_block_locations(&block);
            let IBlock { statements, ret: _ } = block;

            for statement in statements {
                match statement {
                    IntermediateStatement::IntermediateAssignment(assignment) => {
                        if strongly_required_locations.contains(&assignment.location) {
                            self.strongly_process_location(
                                assignment.location,
                                defined,
                                &strongly_required_locations,
                                &mut new_statements,
                            );
                        }
                    }
                }
            }
            self.strongly_process_location(
                memory.location.clone(),
                defined,
                &strongly_required_locations,
                &mut new_statements,
            );
            memory.clone().into()
        } else {
            block.ret
        };
        IBlock {
            statements: new_statements,
            ret,
        }
    }
    fn strongly_process_location(
        &self,
        location: Location,
        defined: &mut HashSet<Location>,
        strongly_required_locations: &HashSet<Location>,
        new_statements: &mut Vec<IntermediateStatement>,
    ) {
        if defined.contains(&location) {
            return;
        }
        defined.insert(location.clone());

        let Some(mut expression) = self.definitions.get(&location).cloned() else {
            return;
        };

        for location in self.strong_expression_locations(&expression) {
            self.strongly_process_location(
                location,
                defined,
                strongly_required_locations,
                new_statements,
            );
        }
        let undefined = strongly_required_locations
            .clone()
            .difference(&defined)
            .cloned()
            .collect_vec();
        defined.extend(undefined.clone());
        match &mut expression {
            IntermediateExpression::ILambda(lambda) => {
                lambda.block = self.strongly_reorder(lambda.block.clone(), defined);
            }
            IntermediateExpression::IIf(if_) => {
                if_.branches.0 = self.strongly_reorder(if_.branches.0.clone(), defined);
                if_.branches.1 = self.strongly_reorder(if_.branches.1.clone(), defined);
            }
            IntermediateExpression::IMatch(match_) => {
                for branch in &mut match_.branches {
                    branch.block = self.strongly_reorder(branch.block.clone(), defined);
                }
            }
            _ => {}
        }
        for undefined in undefined {
            assert!(defined.remove(&undefined));
        }

        new_statements.push(
            IntermediateAssignment {
                location,
                expression,
            }
            .into(),
        );
    }
    fn strong_block_locations(&self, block: &IBlock) -> HashSet<Location> {
        let mut strongly_required_locations =
            HashSet::from_iter(block.ret.filter_memory_location().into_iter());
        for statement in block.statements.iter().rev() {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    if strongly_required_locations.contains(location) {
                        strongly_required_locations
                            .extend(self.strong_expression_locations(expression).into_iter());
                    }
                }
            }
        }
        strongly_required_locations
    }
    pub fn strong_expression_locations(
        &self,
        expression: &IntermediateExpression,
    ) -> Vec<Location> {
        let merge = |a, b| {
            HashSet::intersection(&a, &b)
                .cloned()
                .collect::<HashSet<_>>()
        };
        let values = match &expression {
            IntermediateExpression::ILambda(lambda) => lambda.find_open_vars(),
            IntermediateExpression::IIf(if_) => {
                let required = merge(
                    HashSet::<IntermediateValue>::from_iter(if_.branches.0.values()),
                    HashSet::from_iter(if_.branches.1.values()),
                );
                let mut required = Vec::from_iter(required);
                required.push(if_.condition.clone());
                required
            }
            IntermediateExpression::IMatch(match_) => {
                let mut required = None;
                if match_.branches.len() > 1 {
                    for branch in &match_.branches {
                        let extra = HashSet::from_iter(branch.block.values());
                        required = match required {
                            Some(current) => Some(merge(extra, current)),
                            None => Some(extra),
                        }
                    }
                }
                Vec::from_iter(required.unwrap_or_default())
            }
            expression => expression.values(),
        };
        values
            .iter()
            .filter_map(IntermediateValue::filter_memory_location)
            .collect()
    }

    pub fn eliminate_equivalent_expressions(program: IntermediateProgram) -> IntermediateProgram {
        let IntermediateProgram { main, types } = program;
        let mut optimizer = EquivalentExpressionEliminator::new();
        let lambda = optimizer.eliminate_from_lambda(main);
        let allocation_optimizer = AllocationOptimizer::from_statements(&lambda.block.statements);
        let IntermediateExpression::ILambda(main) =
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
        AllocationOptimizer, AtomicTypeEnum, BuiltInFn, ExpressionEqualityChecker, IIf, ILambda,
        IMatch, Id, Integer, IntermediateArg, IntermediateAssignment, IntermediateBuiltIn,
        IntermediateElementAccess, IntermediateFnCall, IntermediateFnType, IntermediateMatchBranch,
        IntermediateMemory, IntermediateProgram, IntermediateTupleExpression,
        IntermediateTupleType, IntermediateType, IntermediateUnionType, IntermediateValue,
        Location,
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
            let c = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let target = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let zero = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()]);
            let one = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()]);
            let cond = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![
                    IntermediateAssignment{
                        location: target.location.clone(),
                        expression: IIf{
                            condition: cond.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: c.location.clone(),
                                            expression: zero.clone().into()
                                        }.into()
                                    ],
                                    c.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: a.location.clone(),
                                            expression: one.clone().into()
                                        }.into(),
                                        IntermediateAssignment{
                                            location: b.location.clone(),
                                            expression: one.clone().into()
                                        }.into(),
                                    ],
                                    b.clone().into()
                                ).into(),
                            )
                        }.into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        location: target.location.clone(),
                        expression: IIf{
                            condition: cond.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: c.location.clone(),
                                            expression: zero.clone().into()
                                        }.into()
                                    ],
                                    c.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: b.location.clone(),
                                            expression: one.clone().into()
                                        }.into(),
                                    ],
                                    b.clone().into()
                                ).into(),
                            )
                        }.into()
                    }.into()
                ],
                vec![
                    target.location.clone()
                ]
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let w = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(vec![AtomicTypeEnum::INT.into()]).into(),AtomicTypeEnum::INT.into()])));
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(vec![AtomicTypeEnum::INT.into()]).into(),AtomicTypeEnum::INT.into()])));
            let z = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let target = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![IntermediateTupleType(vec![AtomicTypeEnum::INT.into()]).into(),AtomicTypeEnum::INT.into()])));
            let eight = IntermediateTupleExpression(vec![IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 8})).into()]);
            let c = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            (
                vec![
                    IntermediateAssignment{
                        location: target.location.clone(),
                        expression: IIf{
                            condition: c.clone().into(),
                            branches: (
                                (
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
                                        }.into(),
                                    ],
                                    y.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: z.location.clone(),
                                            expression: eight.clone().into()
                                        }.into(),
                                        IntermediateAssignment{
                                            location: w.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                z.clone().into(),
                                                Integer{value: 1}.into(),
                                            ]).into()
                                        }.into(),
                                    ],
                                    w.clone().into()
                                ).into(),
                            )
                        }.into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        location: z.location.clone(),
                        expression: eight.clone().into()
                    }.into(),
                    IntermediateAssignment{
                        location: target.location.clone(),
                        expression: IIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: y.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                z.clone().into(),
                                                Integer{value: 0}.into(),
                                            ]).into()
                                        }.into(),
                                    ],
                                    y.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: w.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                z.clone().into(),
                                                Integer{value: 1}.into(),
                                            ]).into()
                                        }.into(),
                                    ],
                                    w.clone().into()
                                ).into(),
                            )
                        }.into()
                    }.into()
                ],
                vec![
                    target.location.clone()
                ]
            )
        };
        "if statement shared value across branch"
    )]
    #[test_case(
        {
            let a = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let b = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let c = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let x = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let s = IntermediateArg::from(IntermediateType::from(IntermediateUnionType(vec![None, None, Some(AtomicTypeEnum::INT.into())])));
            (
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: a.location.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    Integer{value: 0}.into(),
                                                ]).into()
                                            }.into()
                                        ],
                                        a.clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: b.location.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    Integer{value: 1}.into(),
                                                ]).into()
                                            }.into()
                                        ],
                                        b.clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch{
                                    target: Some(arg.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: c.location.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    arg.clone().into(),
                                                ]).into()
                                            }.into()
                                        ],
                                        c.clone().into(),
                                    ).into()
                                },
                            ]
                        }.into()
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
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: IntermediateValue::from(y.clone()).into()
                                },
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: b.location.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    Integer{value: 1}.into(),
                                                ]).into()
                                            }.into()
                                        ],
                                        b.clone().into()
                                    ).into()
                                },
                                IntermediateMatchBranch{
                                    target: Some(arg.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: c.location.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    arg.clone().into(),
                                                ]).into()
                                            }.into()
                                        ],
                                        c.clone().into(),
                                    ).into()
                                },
                            ]
                        }.into()
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
            let x = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let z = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(vec![AtomicTypeEnum::INT.into()])));
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let s = IntermediateArg::from(IntermediateType::from(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into())])));
            (
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(arg.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                location: y.location.clone(),
                                                expression: IntermediateValue::from(
                                                    arg.clone()
                                                ).into()
                                            }.into(),
                                        ],
                                        y.clone().into()
                                    ).into()
                                },
                            ]
                        }.into(),
                    }.into(),
                    IntermediateAssignment{
                        location: z.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            x.clone().into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(arg.clone()),
                                    block: IntermediateValue::from(arg.clone()).into()
                                },
                            ]
                        }.into(),
                    }.into(),
                    IntermediateAssignment{
                        location: z.location.clone(),
                        expression: IntermediateTupleExpression(vec![
                            x.clone().into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    z.location.clone(),
                ]
            )
        };
        "match statement single branch"
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
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IIf{
                            condition: c.clone().into(),
                            branches: (
                                IntermediateValue::from(y.clone()).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: z.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                Integer{value: 1}.into(),
                                            ]).into()
                                        }.into()
                                    ],
                                    z.clone().into()
                                ).into()
                            )
                        }.into(),
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        location: x.location.clone(),
                        expression: IIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: y.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                Integer{value: 0}.into(),
                                            ]).into()
                                        }.into(),
                                    ],
                                    IntermediateValue::from(y.clone()).into(),
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            location: z.location.clone(),
                                            expression: IntermediateTupleExpression(vec![
                                                Integer{value: 1}.into(),
                                            ]).into()
                                        }.into()
                                    ],
                                    z.clone().into()
                                ).into()
                            )
                        }.into(),
                    }.into(),
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
                    IntermediateAssignment{
                        location: y.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: IntermediateValue::from(
                                        z.clone()
                                    ).into()
                                },
                                IntermediateMatchBranch{
                                    target: None,
                                    block: IntermediateValue::from(
                                        Integer{value: 1}
                                    ).into()
                                }
                            ]
                        }.into(),
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        location: y.location.clone(),
                        expression: IMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
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
                                        ],
                                        IntermediateValue::from(
                                            z.clone()
                                        )
                                    ).into()
                                },
                                IntermediateMatchBranch{
                                    target: None,
                                    block: IntermediateValue::from(
                                        Integer{value: 1}
                                    ).into()
                                }
                            ]
                        }.into(),
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
                        expression: ILambda{
                            args: Vec::new(),
                            block: IBlock {
                                statements: vec![
                                    ret.clone().into()
                                ],
                                ret: ret.clone().into()
                            },
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: ILambda{
                            args: Vec::new(),
                            block: IBlock{
                                statements: Vec::new(),
                                ret: assignment.clone().into()
                            },
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
                        expression: ILambda{
                            args: Vec::new(),
                            block: IBlock {
                                statements: vec![
                                    assignment.clone().into(),
                                    ret.clone().into()
                                ],
                                ret: ret.clone().into()
                            },
                        }.into(),
                        location: lambda.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: ILambda{
                            args: Vec::new(),
                            block: IBlock {
                                statements: vec![
                                    assignment.clone().into(),
                                ],
                                ret: assignment.clone().into()
                            },
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
                        expression: ILambda{
                            args: vec![arg.clone()],
                            block: IBlock {
                                statements: vec![
                                    call.clone().into()
                                ],
                                ret: call.clone().into()
                            },
                        }.into(),
                        location: lambda.location.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: ILambda{
                            args: vec![arg.clone()],
                            block: IBlock {
                                statements: vec![
                                    call.clone().into()
                                ],
                                ret: call.clone().into()
                            },
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
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: ILambda{
                            args: vec![x.clone()],
                            block: IBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: bar_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: ILambda{
                            args: vec![y.clone()],
                            block: IBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: vec![y.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: bar.location.clone()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        expression: ILambda{
                            args: vec![x.clone()],
                            block: IBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: bar_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: ILambda{
                            args: vec![y.clone()],
                            block: IBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: vec![y.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: bar.location.clone()
                    }.into(),
                ],
                vec![
                    foo.location.clone(),
                    bar.location.clone(),
                ]
            )
        };
        "mutually recursive fns"
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
        let expected_fn = ILambda {
            args: Vec::new(),
            block: IBlock {
                statements: expected_statements,
                ret: expected_location.clone().into(),
            },
        };
        let mut equivalent_expression_eliminator = EquivalentExpressionEliminator::new();
        let optimized_fn = equivalent_expression_eliminator.eliminate_from_lambda(ILambda {
            args: Vec::new(),
            block: IBlock {
                statements: original_statements,
                ret: original_location.clone().into(),
            },
        });
        let allocation_optimizer =
            AllocationOptimizer::from_statements(&optimized_fn.block.statements);
        dbg!(&optimized_fn.block.statements);
        let optimized_fn =
            allocation_optimizer.remove_wasted_allocations_from_expression(optimized_fn.into());
        dbg!(&expected_fn);
        dbg!(&optimized_fn);
        ExpressionEqualityChecker::assert_equal(&optimized_fn, &expected_fn.into());
    }

    #[test]
    fn test_refresh_lambdas() {
        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let id = ILambda {
            args: vec![arg.clone()],
            block: IBlock {
                statements: Vec::new(),
                ret: arg.clone().into(),
            },
        };
        let id_loc = IntermediateMemory {
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
            location: Location::new(),
        };
        let target = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let target_0 = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let target_1 = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            location: Location::new(),
        };
        let a = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: BuiltInFn(
                    Id::from("++"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )
                    .into(),
                )
                .into(),
                args: vec![Integer { value: 1 }.into()],
            }
            .into(),
            location: Location::new(),
        };
        let b = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: id_loc.clone().into(),
                args: vec![Integer { value: 2 }.into()],
            }
            .into(),
            location: Location::new(),
        };
        let c = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: BuiltInFn(
                    Id::from("--"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )
                    .into(),
                )
                .into(),
                args: vec![Integer { value: 3 }.into()],
            }
            .into(),
            location: Location::new(),
        };
        let d = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: id_loc.clone().into(),
                args: vec![Integer { value: 4 }.into()],
            }
            .into(),
            location: Location::new(),
        };
        let statements = vec![
            IntermediateAssignment {
                location: id_loc.location.clone(),
                expression: id.clone().into(),
            }
            .into(),
            IntermediateAssignment {
                location: target.location.clone(),
                expression: IIf {
                    condition: IntermediateArg {
                        location: Location::new(),
                        type_: AtomicTypeEnum::BOOL.into(),
                    }
                    .into(),
                    branches: (
                        (
                            vec![IntermediateAssignment {
                                location: target_0.location.clone(),
                                expression: IIf {
                                    condition: IntermediateArg {
                                        location: Location::new(),
                                        type_: AtomicTypeEnum::BOOL.into(),
                                    }
                                    .into(),
                                    branches: (
                                        (vec![a.clone().into()], a.clone().into()).into(),
                                        (vec![b.clone().into()], b.clone().into()).into(),
                                    ),
                                }
                                .into(),
                            }
                            .into()],
                            target_0.clone().into(),
                        )
                            .into(),
                        (
                            vec![IntermediateAssignment {
                                location: target_1.location.clone(),
                                expression: IIf {
                                    condition: IntermediateArg {
                                        location: Location::new(),
                                        type_: AtomicTypeEnum::BOOL.into(),
                                    }
                                    .into(),
                                    branches: (
                                        (vec![c.clone().into()], c.clone().into()).into(),
                                        (vec![d.clone().into()], d.clone().into()).into(),
                                    ),
                                }
                                .into(),
                            }
                            .into()],
                            target_1.clone().into(),
                        )
                            .into(),
                    ),
                }
                .into(),
            }
            .into(),
        ];
        let mut equivalent_expression_eliminator = EquivalentExpressionEliminator::new();
        let optimized_lambda = equivalent_expression_eliminator.eliminate_from_lambda(ILambda {
            args: Vec::new(),
            block: IBlock {
                statements,
                ret: target.clone().into(),
            },
        });
        let optimized_statements = optimized_lambda.block.statements;
        assert_eq!(optimized_statements.len(), 1);
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IIf(IIf {
                    condition: _,
                    branches,
                }),
            location: _,
        }) = optimized_statements[0].clone()
        else {
            panic!()
        };
        assert_eq!(branches.0.statements.len(), 1);
        assert_eq!(branches.1.statements.len(), 1);
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IIf(IIf {
                    condition: _,
                    branches: true_branches,
                }),
            location: _,
        }) = branches.0.statements[0].clone()
        else {
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IIf(IIf {
                    condition: _,
                    branches: false_branches,
                }),
            location: _,
        }) = branches.1.statements[0].clone()
        else {
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression: IntermediateExpression::ILambda(lambda_0),
            location: location_0,
        }) = true_branches.1.statements[0].clone()
        else {
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression: IntermediateExpression::ILambda(lambda_1),
            location: location_1,
        }) = false_branches.1.statements[0].clone()
        else {
            panic!()
        };
        assert_ne!(location_0, location_1);
        assert_ne!(lambda_0.args, lambda_1.args);
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
                    main: ILambda{
                        args: Vec::new(),
                        block: IBlock {
                            statements: vec![
                                assignment.clone().into(),
                                ret.clone().into()
                            ],
                            ret: ret.clone().into()
                        },
                    },
                    types: types.clone()
                },
                IntermediateProgram{
                    main: ILambda{
                        args: Vec::new(),
                        block: IBlock {
                            statements: vec![
                                assignment.clone().into()
                            ],
                            ret: assignment.clone().into()
                        },
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
        ExpressionEqualityChecker::assert_equal(
            &optimized_program.main.into(),
            &expected_program.main.into(),
        )
    }
}
