use std::collections::HashMap;

use crate::intermediate_nodes::*;

pub type MemoryMap = HashMap<Register, IntermediateExpression>;

/// Remove unnecessary assignments and inline built-ins.
pub struct CopyPropagator {
    memory: MemoryMap,
}

impl CopyPropagator {
    /// Instantiate from existing memory map.
    pub fn from_memory_map(memory_map: MemoryMap) -> Self {
        Self { memory: memory_map }
    }
    /// Instantiate from statements.
    pub fn from_statements(statements: &Vec<IntermediateStatement>) -> Self {
        let mut copy_propagator = Self::from_memory_map(MemoryMap::new());
        copy_propagator.register_memory(statements);
        copy_propagator
    }

    /// Record all assignments.
    fn register_memory(&mut self, statements: &Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    match &expression {
                        IntermediateExpression::IntermediateLambda(IntermediateLambda {
                            args: _,
                            block,
                        }) => {
                            self.register_memory(&block.statements);
                        }
                        IntermediateExpression::IntermediateIf(IntermediateIf {
                            condition: _,
                            branches,
                        }) => {
                            self.register_memory(&branches.0.statements);
                            self.register_memory(&branches.1.statements);
                        }
                        IntermediateExpression::IntermediateMatch(IntermediateMatch {
                            subject: _,
                            branches,
                        }) => {
                            for branch in branches {
                                self.register_memory(&branch.block.statements)
                            }
                        }
                        _ => {}
                    }
                    self.memory.insert(register.clone(), expression.clone());
                }
            }
        }
    }

    pub fn propagate_copies_in_expression(
        &self,
        expression: IntermediateExpression,
    ) -> IntermediateExpression {
        match expression {
            IntermediateExpression::IntermediateValue(value) => match value.clone() {
                _ => {
                    IntermediateExpression::IntermediateValue(self.propagate_copies_in_value(value))
                }
            },
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => IntermediateElementAccess {
                value: self.propagate_copies_in_value(value),
                idx,
            }
            .into(),
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => IntermediateTupleExpression(self.propagate_copies_in_values(values)).into(),
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                IntermediateFnCall {
                    fn_: self.propagate_copies_in_value(fn_),
                    args: self.propagate_copies_in_values(args),
                }
                .into()
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => IntermediateCtorCall {
                idx,
                data: data.map(|data| self.propagate_copies_in_value(data)),
                type_,
            }
            .into(),
            IntermediateExpression::IntermediateLambda(IntermediateLambda { args, block }) => {
                IntermediateLambda {
                    args,
                    block: self.propagate_copies_in_block(block),
                }
                .into()
            }
            IntermediateExpression::IntermediateIf(IntermediateIf {
                condition,
                branches,
            }) => IntermediateIf {
                condition: self.propagate_copies_in_value(condition),
                branches: (
                    self.propagate_copies_in_block(branches.0),
                    self.propagate_copies_in_block(branches.1),
                ),
            }
            .into(),
            IntermediateExpression::IntermediateMatch(IntermediateMatch { subject, branches }) => {
                IntermediateMatch {
                    subject: self.propagate_copies_in_value(subject),
                    branches: branches
                        .into_iter()
                        .map(
                            |IntermediateMatchBranch { target, block }| IntermediateMatchBranch {
                                target,
                                block: self.propagate_copies_in_block(block),
                            },
                        )
                        .collect(),
                }
                .into()
            }
        }
    }
    pub fn propagate_copies_in_value(&self, value: IntermediateValue) -> IntermediateValue {
        match value.clone() {
            IntermediateValue::IntermediateBuiltIn(built_in) => built_in.into(),
            IntermediateValue::IntermediateArg(arg) => arg.into(),
            IntermediateValue::IntermediateMemory(memory) => {
                // Inline assignment if possible.
                if let Some(expression) = self.memory.get(&memory.register) {
                    match expression {
                        IntermediateExpression::IntermediateValue(value) => {
                            // Inline value recursively.
                            self.propagate_copies_in_value(value.clone())
                        }
                        _ => memory.into(),
                    }
                } else {
                    memory.into()
                }
            }
        }
    }
    pub fn propagate_copies_in_values(
        &self,
        values: Vec<IntermediateValue>,
    ) -> Vec<IntermediateValue> {
        values
            .into_iter()
            .map(|value| self.propagate_copies_in_value(value))
            .collect()
    }
    pub fn propagate_copies_in_statement(
        &self,
        statement: IntermediateStatement,
    ) -> Option<IntermediateStatement> {
        match statement {
            IntermediateStatement::IntermediateAssignment(assignment) => {
                let IntermediateAssignment {
                    expression,
                    register,
                } = assignment;
                // Remove assignments to a value.
                if matches!(&expression, IntermediateExpression::IntermediateValue(_)) {
                    return None;
                }
                let condensed_expression = self.propagate_copies_in_expression(expression.clone());
                let expression = condensed_expression;
                Some(IntermediateStatement::IntermediateAssignment(
                    IntermediateAssignment {
                        register,
                        expression,
                    }
                    .into(),
                ))
            }
        }
    }
    pub fn propagate_copies_in_statements(
        &self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        statements
            .into_iter()
            .filter_map(|statement| self.propagate_copies_in_statement(statement))
            .collect()
    }
    pub fn propagate_copies_in_block(
        &self,
        IntermediateBlock { statements, ret }: IntermediateBlock,
    ) -> IntermediateBlock {
        IntermediateBlock {
            statements: self.propagate_copies_in_statements(statements),
            ret: self.propagate_copies_in_value(ret),
        }
    }
}
