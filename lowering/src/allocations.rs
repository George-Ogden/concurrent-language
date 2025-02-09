use std::collections::HashMap;

use crate::intermediate_nodes::*;

pub type MemoryMap = HashMap<Location, Vec<IntermediateExpression>>;

pub struct AllocationOptimizer {
    memory: MemoryMap,
}
impl AllocationOptimizer {
    pub fn new(memory_map: MemoryMap) -> Self {
        Self { memory: memory_map }
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
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args,
                statements,
                ret,
            }) => IntermediateLambda {
                args,
                statements: self.remove_wasted_allocations_from_statements(statements),
                ret: self.remove_wasted_allocations_from_value(ret),
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
                        expression: expression,
                    }
                    .into(),
                ))
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => Some(
                IntermediateIfStatement {
                    condition: self.remove_wasted_allocations_from_value(condition),
                    branches: (
                        self.remove_wasted_allocations_from_statements(branches.0),
                        self.remove_wasted_allocations_from_statements(branches.1),
                    ),
                }
                .into(),
            ),
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => Some(
                IntermediateMatchStatement {
                    subject: self.remove_wasted_allocations_from_value(subject),
                    branches: branches
                        .into_iter()
                        .map(|IntermediateMatchBranch { target, statements }| {
                            IntermediateMatchBranch {
                                target,
                                statements: self
                                    .remove_wasted_allocations_from_statements(statements),
                            }
                        })
                        .collect(),
                }
                .into(),
            ),
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
}
