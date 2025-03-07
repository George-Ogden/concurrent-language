use std::collections::HashMap;

use crate::intermediate_nodes::*;

pub type MemoryMap = HashMap<Location, Vec<IntermediateExpression>>;

pub struct AllocationOptimizer {
    memory: MemoryMap,
}
impl AllocationOptimizer {
    pub fn from_memory_map(memory_map: MemoryMap) -> Self {
        Self { memory: memory_map }
    }
    pub fn from_statements(statements: &Vec<IntermediateStatement>) -> Self {
        let mut allocation_optimizer = Self::from_memory_map(MemoryMap::new());
        allocation_optimizer.register_memory(statements);
        allocation_optimizer
    }

    fn register_memory(&mut self, statements: &Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    match &expression {
                        IntermediateExpression::ILambda(ILambda { args: _, block }) => {
                            self.register_memory(&block.statements);
                        }
                        IntermediateExpression::IIf(IIf {
                            condition: _,
                            branches,
                        }) => {
                            self.register_memory(&branches.0.statements);
                            self.register_memory(&branches.1.statements);
                        }
                        IntermediateExpression::IMatch(IMatch {
                            subject: _,
                            branches,
                        }) => {
                            for branch in branches {
                                self.register_memory(&branch.block.statements)
                            }
                        }
                        _ => {}
                    }
                    if !self.memory.contains_key(&location) {
                        self.memory.insert(location.clone(), Vec::new());
                    }
                    self.memory
                        .get_mut(&location)
                        .unwrap()
                        .push(expression.clone());
                }
            }
        }
    }

    pub fn remove_wasted_allocations_from_expression(
        &self,
        expression: IntermediateExpression,
    ) -> IntermediateExpression {
        match expression {
            IntermediateExpression::IntermediateValue(value) => match value.clone() {
                _ => IntermediateExpression::IntermediateValue(
                    self.remove_wasted_allocations_from_value(value),
                ),
            },
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => IntermediateElementAccess {
                value: self.remove_wasted_allocations_from_value(value),
                idx,
            }
            .into(),
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => IntermediateTupleExpression(self.remove_wasted_allocations_from_values(values))
                .into(),
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                IntermediateFnCall {
                    fn_: self.remove_wasted_allocations_from_value(fn_),
                    args: self.remove_wasted_allocations_from_values(args),
                }
                .into()
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => IntermediateCtorCall {
                idx,
                data: data.map(|data| self.remove_wasted_allocations_from_value(data)),
                type_,
            }
            .into(),
            IntermediateExpression::ILambda(ILambda { args, block }) => ILambda {
                args,
                block: self.remove_wasted_allocations_from_block(block),
            }
            .into(),
            IntermediateExpression::IIf(IIf {
                condition,
                branches,
            }) => IIf {
                condition: self.remove_wasted_allocations_from_value(condition),
                branches: (
                    self.remove_wasted_allocations_from_block(branches.0),
                    self.remove_wasted_allocations_from_block(branches.1),
                ),
            }
            .into(),
            IntermediateExpression::IMatch(IMatch { subject, branches }) => IMatch {
                subject: self.remove_wasted_allocations_from_value(subject),
                branches: branches
                    .into_iter()
                    .map(
                        |IntermediateMatchBranch { target, block }| IntermediateMatchBranch {
                            target,
                            block: self.remove_wasted_allocations_from_block(block),
                        },
                    )
                    .collect(),
            }
            .into(),
        }
    }
    pub fn remove_wasted_allocations_from_value(
        &self,
        value: IntermediateValue,
    ) -> IntermediateValue {
        match value.clone() {
            IntermediateValue::IntermediateBuiltIn(built_in) => built_in.into(),
            IntermediateValue::IntermediateArg(arg) => arg.into(),
            IntermediateValue::IntermediateMemory(memory) => {
                let expressions = self.memory.get(&memory.location);
                if expressions.map(Vec::len) == Some(1) {
                    let expressions = expressions.unwrap();
                    let expression = expressions[0].clone();
                    match expression {
                        IntermediateExpression::IntermediateValue(value) => {
                            self.remove_wasted_allocations_from_value(value)
                        }
                        _ => memory.into(),
                    }
                } else {
                    memory.into()
                }
            }
        }
    }
    pub fn remove_wasted_allocations_from_values(
        &self,
        values: Vec<IntermediateValue>,
    ) -> Vec<IntermediateValue> {
        values
            .into_iter()
            .map(|value| self.remove_wasted_allocations_from_value(value))
            .collect()
    }
    pub fn remove_wasted_allocations_from_statement(
        &self,
        statement: IntermediateStatement,
    ) -> Option<IntermediateStatement> {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                let IntermediateAssignment {
                    expression,
                    location,
                } = assignment;
                if matches!(&expression, IntermediateExpression::IntermediateValue(_))
                    && self.memory.get(&location).map(Vec::len) == Some(1)
                {
                    return None;
                }
                let condensed_expression =
                    self.remove_wasted_allocations_from_expression(expression.clone());
                let expression = condensed_expression;
                Some(IntermediateStatement::IntermediateAssignment(
                    IntermediateAssignment {
                        location,
                        expression,
                    }
                    .into(),
                ))
            }
        }
    }
    pub fn remove_wasted_allocations_from_statements(
        &self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        statements
            .into_iter()
            .filter_map(|statement| self.remove_wasted_allocations_from_statement(statement))
            .collect()
    }
    pub fn remove_wasted_allocations_from_block(
        &self,
        IBlock { statements, ret }: IBlock,
    ) -> IBlock {
        IBlock {
            statements: self.remove_wasted_allocations_from_statements(statements),
            ret: self.remove_wasted_allocations_from_value(ret),
        }
    }
}
