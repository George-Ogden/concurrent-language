use std::collections::{HashMap, HashSet};

use lowering::{
    CopyPropagator, IntermediateAssignment, IntermediateBlock, IntermediateExpression,
    IntermediateLambda, IntermediateMemory, IntermediateProgram, IntermediateStatement,
    IntermediateValue, Register,
};

use crate::refresher::Refresher;

type HistoricalExpressions = HashMap<IntermediateExpression, Register>;
type Definitions = HashMap<Register, IntermediateExpression>;
type NormalizedRegisters = HashMap<Register, Register>;

#[derive(Clone)]
pub struct RedundancyEliminator {
    historical_expressions: HistoricalExpressions,
    definitions: Definitions,
    normalized_registers: NormalizedRegisters,
}

impl RedundancyEliminator {
    pub fn new() -> Self {
        Self {
            historical_expressions: HistoricalExpressions::new(),
            normalized_registers: NormalizedRegisters::new(),
            definitions: Definitions::new(),
        }
    }

    /// Normalize equivalent registers in an expression.
    fn normalize_expression(
        &self,
        mut expression: IntermediateExpression,
    ) -> IntermediateExpression {
        expression.substitute(&self.normalized_registers);
        expression
    }

    fn eliminate_from_lambda(&mut self, lambda: IntermediateLambda) -> IntermediateLambda {
        let IntermediateLambda { args, mut block } = lambda;
        self.prepare_history(&mut block);
        let block = self.weakly_reorder(block, &mut HashSet::new());

        let mut lambda = IntermediateLambda { args, block };
        Refresher::refresh(&mut lambda);

        let IntermediateLambda { args, block } = lambda;
        self.refresh_history(&block);
        let block = self.strongly_reorder(block, &mut HashSet::new());

        let mut lambda = IntermediateLambda { args, block };
        Refresher::refresh(&mut lambda);
        lambda
    }
    /// Store normalized expressions and their registers.
    fn prepare_history(&mut self, block: &mut IntermediateBlock) {
        for statement in &mut block.statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    *expression = self.normalize_expression(expression.clone());
                    match expression {
                        IntermediateExpression::IntermediateLambda(ref mut lambda) => {
                            self.prepare_history(&mut lambda.block);
                        }
                        IntermediateExpression::IntermediateIf(ref mut if_) => {
                            for block in [&mut if_.branches.0, &mut if_.branches.1] {
                                self.prepare_history(block);
                            }
                        }
                        IntermediateExpression::IntermediateMatch(ref mut match_) => {
                            for branch in &mut match_.branches {
                                self.prepare_history(&mut branch.block);
                            }
                        }
                        _ => {}
                    }
                    // Check whether the expression has already been defined.
                    let new_register = match self.historical_expressions.get(&expression) {
                        None => {
                            self.historical_expressions
                                .insert(expression.clone(), register.clone());
                            register.clone()
                        }
                        Some(new_register) => new_register.clone(),
                    };
                    // Store the assignment in the definitions.
                    self.definitions
                        .insert(register.clone(), expression.clone());
                    // Add a normalized register if there is an equivalent assignment (otherwise this is the same as `register`).
                    self.normalized_registers
                        .insert(register.clone(), new_register);
                }
            }
        }
        block.ret = block.ret.substitute(&self.normalized_registers);
    }
    /// Update the history with new definitions.
    fn refresh_history(&mut self, block: &IntermediateBlock) {
        for statement in &block.statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    match &expression {
                        IntermediateExpression::IntermediateLambda(ref lambda) => {
                            self.refresh_history(&lambda.block);
                        }
                        IntermediateExpression::IntermediateIf(ref if_) => {
                            for block in [&if_.branches.0, &if_.branches.1] {
                                self.refresh_history(&block);
                            }
                        }
                        IntermediateExpression::IntermediateMatch(ref match_) => {
                            for branch in &match_.branches {
                                self.refresh_history(&branch.block);
                            }
                        }
                        _ => {}
                    }
                    self.definitions
                        .insert(register.clone(), expression.clone());
                }
            }
        }
    }
    /// Reorder statements based on weak requirements.
    fn weakly_reorder(
        &self,
        block: IntermediateBlock,
        defined: &mut HashSet<Register>,
    ) -> IntermediateBlock {
        let mut new_statements = Vec::new();
        let weakly_required_registers = self.weak_block_registers(&block);

        let IntermediateBlock { statements, ret } = block;

        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(assignment) => {
                    self.weakly_process_register(
                        assignment.register,
                        defined,
                        &weakly_required_registers,
                        &mut new_statements,
                    );
                }
            }
        }
        if let Some(register) = ret.filter_memory_register() {
            self.weakly_process_register(
                register,
                defined,
                &weakly_required_registers,
                &mut new_statements,
            );
        }
        IntermediateBlock {
            statements: new_statements,
            ret,
        }
    }
    fn weakly_process_register(
        &self,
        register: Register,
        defined: &mut HashSet<Register>,
        weakly_required_registers: &HashSet<Register>,
        new_statements: &mut Vec<IntermediateStatement>,
    ) {
        // Ensure the register has not already been defined and is weakly required.
        if defined.contains(&register) || !weakly_required_registers.contains(&register) {
            return;
        }
        defined.insert(register.clone());

        let Some(mut expression) = self.definitions.get(&register).cloned() else {
            return;
        };

        // Consider any register that might be used and whether it should be used.
        for register in self.very_weak_expression_registers(&expression) {
            self.weakly_process_register(
                register,
                defined,
                weakly_required_registers,
                new_statements,
            );
        }
        match &mut expression {
            IntermediateExpression::IntermediateLambda(lambda) => {
                lambda.block = self.weakly_reorder(lambda.block.clone(), &mut defined.clone());
            }
            IntermediateExpression::IntermediateIf(if_) => {
                if_.branches.0 = self.weakly_reorder(if_.branches.0.clone(), &mut defined.clone());
                if_.branches.1 = self.weakly_reorder(if_.branches.1.clone(), &mut defined.clone());
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                for branch in &mut match_.branches {
                    branch.block = self.weakly_reorder(branch.block.clone(), &mut defined.clone());
                }
            }
            _ => {}
        }

        new_statements.push(
            IntermediateAssignment {
                register,
                expression,
            }
            .into(),
        );
    }
    fn weak_block_registers(&self, block: &IntermediateBlock) -> HashSet<Register> {
        // Weak block registers are those that are already assigned to or are needed by all future paths in the program.
        let mut registers =
            HashSet::from_iter(
                block
                    .statements
                    .iter()
                    .flat_map(|statement| match statement {
                        IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                            expression,
                            register,
                        }) => {
                            // Lookup normalized register.
                            let (register, expression) =
                                if let IntermediateExpression::IntermediateValue(
                                    IntermediateValue::IntermediateMemory(IntermediateMemory {
                                        type_: _,
                                        register: normalized_register,
                                    }),
                                ) = expression
                                {
                                    let expression = self.definitions[&normalized_register].clone();
                                    (normalized_register.clone(), expression)
                                } else {
                                    (register.clone(), expression.clone())
                                };
                            // Determine all weakly used registers.
                            let mut registers = vec![register.clone()];
                            registers
                                .extend(self.weak_expression_registers(&expression).into_iter());
                            registers
                        }
                    }),
            );
        registers.extend(block.ret.filter_memory_register().into_iter());
        registers
    }
    pub fn weak_expression_registers(
        &self,
        expression: &IntermediateExpression,
    ) -> HashSet<Register> {
        let merge = |a, b| {
            HashSet::intersection(&a, &b)
                .cloned()
                .collect::<HashSet<_>>()
        };
        match &expression {
            // Only open variables are weakly required in lambdas.
            IntermediateExpression::IntermediateLambda(lambda) => lambda
                .find_open_vars()
                .iter()
                .filter_map(IntermediateValue::filter_memory_register)
                .collect(),
            // Weakly required variables must be weakly required in both branches.
            IntermediateExpression::IntermediateIf(if_) => {
                let mut required = merge(
                    self.weak_block_registers(&if_.branches.0),
                    self.weak_block_registers(&if_.branches.1),
                );
                required.extend(if_.condition.filter_memory_register().into_iter());
                required
            }
            // Weakly required variables must be weakly required in all branches.
            IntermediateExpression::IntermediateMatch(match_) => {
                let mut required = None;
                // Do not move expressions out of a single match branch.
                if match_.branches.len() > 1 {
                    for branch in &match_.branches {
                        let extra = self.weak_block_registers(&branch.block);
                        required = match required {
                            Some(current) => Some(merge(extra, current)),
                            None => Some(extra),
                        }
                    }
                }
                let mut required = required.unwrap_or_default();
                required.extend(match_.subject.filter_memory_register().into_iter());
                required
            }
            expression => expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_register)
                .collect(),
        }
    }
    /// Determine all registers that may be used in an expression.
    pub fn very_weak_expression_registers(
        &self,
        expression: &IntermediateExpression,
    ) -> HashSet<Register> {
        match &expression {
            IntermediateExpression::IntermediateLambda(lambda) => {
                self.very_weak_block_registers(&lambda.block)
            }
            IntermediateExpression::IntermediateIf(if_) => {
                let mut required = self.very_weak_block_registers(&if_.branches.0);
                required.extend(self.very_weak_block_registers(&if_.branches.1));
                required.extend(if_.condition.filter_memory_register().into_iter());
                required
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                let mut required = match_
                    .branches
                    .iter()
                    .flat_map(|branch| self.very_weak_block_registers(&branch.block))
                    .collect::<HashSet<_>>();
                required.extend(match_.subject.filter_memory_register().into_iter());
                required
            }
            expression => expression
                .values()
                .iter()
                .filter_map(IntermediateValue::filter_memory_register)
                .collect(),
        }
    }
    fn very_weak_block_registers(&self, block: &IntermediateBlock) -> HashSet<Register> {
        let mut registers =
            HashSet::from_iter(
                block
                    .statements
                    .iter()
                    .flat_map(|statement| match statement {
                        IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                            expression,
                            register,
                        }) => {
                            let (register, expression) =
                                if let IntermediateExpression::IntermediateValue(
                                    IntermediateValue::IntermediateMemory(IntermediateMemory {
                                        type_: _,
                                        register: normalized_register,
                                    }),
                                ) = expression
                                {
                                    let expression = self.definitions[&normalized_register].clone();
                                    (normalized_register.clone(), expression)
                                } else {
                                    (register.clone(), expression.clone())
                                };
                            let mut registers = vec![register.clone()];
                            registers.extend(
                                self.very_weak_expression_registers(&expression).into_iter(),
                            );
                            registers
                        }
                    }),
            );
        registers.extend(block.ret.filter_memory_register().into_iter());
        registers
    }

    /// Reorder statements based on strong requirements.
    fn strongly_reorder(
        &self,
        block: IntermediateBlock,
        defined: &mut HashSet<Register>,
    ) -> IntermediateBlock {
        let mut new_statements = Vec::new();
        let ret = if let IntermediateValue::IntermediateMemory(memory) = &block.ret {
            let strongly_required_registers = self.strong_block_registers(&block);
            let IntermediateBlock { statements, ret: _ } = block;

            for statement in statements {
                match statement {
                    IntermediateStatement::IntermediateAssignment(assignment) => {
                        // Ensure register is strongly defined before processing.
                        if strongly_required_registers.contains(&assignment.register) {
                            self.strongly_process_register(
                                assignment.register,
                                defined,
                                &strongly_required_registers,
                                &mut new_statements,
                            );
                        }
                    }
                }
            }
            self.strongly_process_register(
                memory.register.clone(),
                defined,
                &strongly_required_registers,
                &mut new_statements,
            );
            memory.clone().into()
        } else {
            block.ret
        };
        IntermediateBlock {
            statements: new_statements,
            ret,
        }
    }
    fn strongly_process_register(
        &self,
        register: Register,
        defined: &mut HashSet<Register>,
        strongly_required_registers: &HashSet<Register>,
        new_statements: &mut Vec<IntermediateStatement>,
    ) {
        if defined.contains(&register) {
            return;
        }
        defined.insert(register.clone());

        // If register is not included in definitions, ignore it (mostly used for testing with open variables).
        let Some(mut expression) = self.definitions.get(&register).cloned() else {
            return;
        };

        for register in self.strong_expression_registers(&expression) {
            self.strongly_process_register(
                register,
                defined,
                strongly_required_registers,
                new_statements,
            );
        }
        match &mut expression {
            IntermediateExpression::IntermediateLambda(lambda) => {
                lambda.block = self.strongly_reorder(lambda.block.clone(), &mut defined.clone());
            }
            IntermediateExpression::IntermediateIf(if_) => {
                if_.branches.0 =
                    self.strongly_reorder(if_.branches.0.clone(), &mut defined.clone());
                if_.branches.1 =
                    self.strongly_reorder(if_.branches.1.clone(), &mut defined.clone());
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                for branch in &mut match_.branches {
                    branch.block =
                        self.strongly_reorder(branch.block.clone(), &mut defined.clone());
                }
            }
            _ => {}
        }

        new_statements.push(
            IntermediateAssignment {
                register,
                expression,
            }
            .into(),
        );
    }
    fn strong_block_registers(&self, block: &IntermediateBlock) -> HashSet<Register> {
        // Strong block registers are those that are needed by all future paths in the program.
        let mut strongly_required_registers =
            HashSet::from_iter(block.ret.filter_memory_register().into_iter());
        for statement in block.statements.iter().rev() {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    register,
                }) => {
                    // Reverse registers and use the transitivity to find other strongly required registers.
                    if strongly_required_registers.contains(register) {
                        strongly_required_registers
                            .extend(self.strong_expression_registers(expression).into_iter());
                    }
                }
            }
        }
        strongly_required_registers
    }
    pub fn strong_expression_registers(
        &self,
        expression: &IntermediateExpression,
    ) -> HashSet<Register> {
        let merge = |a, b| {
            HashSet::intersection(&a, &b)
                .cloned()
                .collect::<HashSet<_>>()
        };
        match &expression {
            IntermediateExpression::IntermediateIf(if_) => {
                let mut required = merge(
                    self.weak_block_registers(&if_.branches.0),
                    self.weak_block_registers(&if_.branches.1),
                );
                required.extend(if_.condition.filter_memory_register().into_iter());
                required
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                let mut required = None;
                // Do not move expressions out of a single match branch.
                if match_.branches.len() > 1 {
                    for branch in &match_.branches {
                        let extra = self.weak_block_registers(&branch.block);
                        required = match required {
                            Some(current) => Some(merge(extra, current)),
                            None => Some(extra),
                        }
                    }
                }
                let mut required = required.unwrap_or_default();
                required.extend(match_.subject.filter_memory_register().into_iter());
                required
            }
            expression => self.weak_expression_registers(&expression),
        }
    }

    pub fn eliminate_redundancy(program: IntermediateProgram) -> IntermediateProgram {
        let IntermediateProgram { main, types } = program;
        let mut optimizer = RedundancyEliminator::new();
        let lambda = optimizer.eliminate_from_lambda(main);
        let copy_propagator = CopyPropagator::from_statements(&lambda.block.statements);
        let IntermediateExpression::IntermediateLambda(main) =
            copy_propagator.propagate_copies_in_expression(lambda.into())
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
        AtomicTypeEnum, Boolean, BuiltInFn, CopyPropagator, ExpressionEqualityChecker, Id, Integer,
        IntermediateArg, IntermediateAssignment, IntermediateBuiltIn, IntermediateElementAccess,
        IntermediateFnCall, IntermediateFnType, IntermediateIf, IntermediateLambda,
        IntermediateMatch, IntermediateMatchBranch, IntermediateMemory, IntermediateProgram,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateUnionType, IntermediateValue, Register,
    };
    use test_case::test_case;

    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                register: Register::new()
            };
            (
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: expression.clone().into(),
                        register: Register::new()
                    }.into()
                ],
                vec![
                    assignment.clone().into(),
                ],
                vec![
                    assignment.register.clone()
                ]
            )
        };
        "repeated empty tuple assignment"
    )]
    #[test_case(
        {
            let empty_register_0 = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
            let empty_register_1 = IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
            let empty_assignment_0 = IntermediateAssignment{
                expression: IntermediateTupleExpression(Vec::new()).into(),
                register: empty_register_0.register.clone()
            };
            let empty_assignment_1 = IntermediateAssignment{
                expression: IntermediateTupleExpression(Vec::new()).into(),
                register: empty_register_1.register.clone()
            };
            let nested_assignment_0 = IntermediateAssignment{
                expression: IntermediateTupleExpression(vec![empty_register_0.clone().into()]).into(),
                register: Register::new()
            };
            let nested_assignment_1 = IntermediateAssignment{
                expression: IntermediateTupleExpression(vec![empty_register_1.clone().into()]).into(),
                register: Register::new()
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
                    nested_assignment_0.register.clone(),
                    nested_assignment_0.register.clone()
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
                        register: target.register.clone(),
                        expression: IntermediateIf{
                            condition: cond.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: c.register.clone(),
                                            expression: zero.clone().into()
                                        }.into()
                                    ],
                                    c.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: a.register.clone(),
                                            expression: one.clone().into()
                                        }.into(),
                                        IntermediateAssignment{
                                            register: b.register.clone(),
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
                        register: target.register.clone(),
                        expression: IntermediateIf{
                            condition: cond.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: c.register.clone(),
                                            expression: zero.clone().into()
                                        }.into()
                                    ],
                                    c.clone().into()
                                ).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: b.register.clone(),
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
                    target.register.clone()
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
                        register: target.register.clone(),
                        expression: IntermediateIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: x.register.clone(),
                                            expression: eight.clone().into()
                                        }.into(),
                                        IntermediateAssignment{
                                            register: y.register.clone(),
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
                                            register: z.register.clone(),
                                            expression: eight.clone().into()
                                        }.into(),
                                        IntermediateAssignment{
                                            register: w.register.clone(),
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
                        register: z.register.clone(),
                        expression: eight.clone().into()
                    }.into(),
                    IntermediateAssignment{
                        register: target.register.clone(),
                        expression: IntermediateIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: y.register.clone(),
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
                                            register: w.register.clone(),
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
                    target.register.clone()
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
                        register: x.register.clone(),
                        expression: IntermediateMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                register: a.register.clone(),
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
                                                register: b.register.clone(),
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
                                                register: c.register.clone(),
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
                        register: y.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        register: y.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateAssignment{
                        register: x.register.clone(),
                        expression: IntermediateMatch{
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
                                                register: b.register.clone(),
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
                                                register: c.register.clone(),
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
                    x.register.clone(),
                    y.register.clone(),
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
                        register: x.register.clone(),
                        expression: IntermediateMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: Some(arg.clone()),
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                register: y.register.clone(),
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
                        register: z.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            x.clone().into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        register: x.register.clone(),
                        expression: IntermediateMatch{
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
                        register: z.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            x.clone().into(),
                        ]).into()
                    }.into()
                ],
                vec![
                    z.register.clone(),
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
                        register: y.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateAssignment{
                        register: x.register.clone(),
                        expression: IntermediateIf{
                            condition: c.clone().into(),
                            branches: (
                                IntermediateValue::from(y.clone()).into(),
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: z.register.clone(),
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
                        register: x.register.clone(),
                        expression: IntermediateIf{
                            condition: c.clone().into(),
                            branches: (
                                (
                                    vec![
                                        IntermediateAssignment{
                                            register: y.register.clone(),
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
                                            register: z.register.clone(),
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
                    x.register.clone()
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
                        register: x.register.clone(),
                        expression: IntermediateTupleExpression(vec![
                            Integer{value: 0}.into(),
                        ]).into()
                    }.into(),
                    IntermediateAssignment{
                        register: z.register.clone(),
                        expression: IntermediateElementAccess{
                            value: x.clone().into(),
                            idx: 0
                        }.into()
                    }.into(),
                    IntermediateAssignment{
                        register: y.register.clone(),
                        expression: IntermediateMatch{
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
                        register: y.register.clone(),
                        expression: IntermediateMatch{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch{
                                    target: None,
                                    block: (
                                        vec![
                                            IntermediateAssignment{
                                                register: x.register.clone(),
                                                expression: IntermediateTupleExpression(vec![
                                                    Integer{value: 0}.into(),
                                                ]).into()
                                            }.into(),
                                            IntermediateAssignment{
                                                register: z.register.clone(),
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
                    y.register.clone()
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
                register: Register::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                register: Register::new()
            };
            let lambda = Register::new();
            (
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    ret.clone().into()
                                ],
                                ret: ret.clone().into()
                            },
                        }.into(),
                        register: lambda.clone()
                    }.into()
                ],
                vec![
                    assignment.clone().into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock{
                                statements: Vec::new(),
                                ret: assignment.clone().into()
                            },
                        }.into(),
                        register: lambda.clone()
                    }.into()
                ],
                vec![
                    assignment.register.clone(),
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
                register: Register::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                register: Register::new()
            };
            let lambda = Register::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    assignment.clone().into(),
                                    ret.clone().into()
                                ],
                                ret: ret.clone().into()
                            },
                        }.into(),
                        register: lambda.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    assignment.clone().into(),
                                ],
                                ret: assignment.clone().into()
                            },
                        }.into(),
                        register: lambda.clone()
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
                register: Register::new(),
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
                            block: IntermediateBlock {
                                statements: vec![
                                    call.clone().into()
                                ],
                                ret: call.clone().into()
                            },
                        }.into(),
                        register: lambda.register.clone()
                    }.into()
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![arg.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    call.clone().into()
                                ],
                                ret: call.clone().into()
                            },
                        }.into(),
                        register: lambda.register.clone()
                    }.into()
                ],
                vec![
                    lambda.register.clone()
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
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: bar_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: foo_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: vec![y.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        register: bar.register.clone()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: bar_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: foo_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: vec![y.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        register: bar.register.clone()
                    }.into(),
                ],
                vec![
                    foo.register.clone(),
                    bar.register.clone(),
                ]
            )
        };
        "mutually recursive fns"
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
            let branch = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: bar_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: branch.register.clone(),
                                        expression: IntermediateIf{
                                            condition: Boolean{value: true}.into(),
                                            branches: (
                                                (
                                                    vec![
                                                        IntermediateAssignment{
                                                            register: foo_call.register.clone(),
                                                            expression: IntermediateFnCall{
                                                                fn_: foo.clone().into(),
                                                                args: vec![y.clone().into()]
                                                            }.into()
                                                        }.into()
                                                    ],
                                                    IntermediateValue::from(foo_call.clone()).clone().into()
                                                ).into(),
                                                IntermediateValue::from(Integer{value: 0}).into(),
                                            )
                                        }.into()
                                    }.into(),
                                ],
                                ret: branch.clone().into()
                            },
                        }.into(),
                        register: bar.register.clone()
                    }.into(),
                ],
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: bar_call.register.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: vec![x.clone().into()]
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        register: foo.register.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        register: branch.register.clone(),
                                        expression: IntermediateIf{
                                            condition: Boolean{value: true}.into(),
                                            branches: (
                                                (
                                                    vec![
                                                        IntermediateAssignment{
                                                            register: foo_call.register.clone(),
                                                            expression: IntermediateFnCall{
                                                                fn_: foo.clone().into(),
                                                                args: vec![y.clone().into()]
                                                            }.into()
                                                        }.into()
                                                    ],
                                                    IntermediateValue::from(foo_call.clone()).clone().into()
                                                ).into(),
                                                IntermediateValue::from(Integer{value: 0}).into(),
                                            )
                                        }.into()
                                    }.into(),
                                ],
                                ret: branch.clone().into()
                            },
                        }.into(),
                        register: bar.register.clone()
                    }.into(),
                ],
                vec![
                    foo.register.clone(),
                    bar.register.clone(),
                ]
            )
        };
        "mutually recursive conditional fns"
    )]
    fn test_eliminate(
        statements: (
            Vec<IntermediateStatement>,
            Vec<IntermediateStatement>,
            Vec<Register>,
        ),
    ) {
        let (mut original_statements, mut expected_statements, required) = statements;
        let original_register =
            IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
        let expected_register =
            IntermediateMemory::from(IntermediateType::from(IntermediateTupleType(Vec::new())));
        let return_expression = IntermediateTupleExpression(
            required
                .into_iter()
                .map(|register| {
                    IntermediateMemory {
                        type_: IntermediateTupleType(Vec::new()).into(),
                        register,
                    }
                    .into()
                })
                .collect(),
        );
        original_statements.push(
            IntermediateAssignment {
                register: original_register.register.clone(),
                expression: return_expression.clone().into(),
            }
            .into(),
        );
        expected_statements.push(
            IntermediateAssignment {
                register: expected_register.register.clone(),
                expression: return_expression.clone().into(),
            }
            .into(),
        );
        let expected_fn = IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: expected_statements,
                ret: expected_register.clone().into(),
            },
        };
        let mut redundancy_eliminator = RedundancyEliminator::new();
        let optimized_fn = redundancy_eliminator.eliminate_from_lambda(IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements: original_statements,
                ret: original_register.clone().into(),
            },
        });
        let copy_propagator = CopyPropagator::from_statements(&optimized_fn.block.statements);
        dbg!(&optimized_fn.block.statements);
        let optimized_fn = copy_propagator.propagate_copies_in_expression(optimized_fn.into());
        dbg!(&expected_fn);
        dbg!(&optimized_fn);
        ExpressionEqualityChecker::assert_equal(&optimized_fn, &expected_fn.into());
    }

    #[test]
    fn test_refresh_lambdas() {
        let arg = IntermediateArg {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let id = IntermediateLambda {
            args: vec![arg.clone()],
            block: IntermediateBlock {
                statements: Vec::new(),
                ret: arg.clone().into(),
            },
        };
        let id_reg = IntermediateMemory {
            type_: IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )
            .into(),
            register: Register::new(),
        };
        let target = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let target_0 = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
        };
        let target_1 = IntermediateMemory {
            type_: AtomicTypeEnum::INT.into(),
            register: Register::new(),
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
            register: Register::new(),
        };
        let b = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: id_reg.clone().into(),
                args: vec![Integer { value: 2 }.into()],
            }
            .into(),
            register: Register::new(),
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
            register: Register::new(),
        };
        let d = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: id_reg.clone().into(),
                args: vec![Integer { value: 4 }.into()],
            }
            .into(),
            register: Register::new(),
        };
        let statements = vec![
            IntermediateAssignment {
                register: id_reg.register.clone(),
                expression: id.clone().into(),
            }
            .into(),
            IntermediateAssignment {
                register: target.register.clone(),
                expression: IntermediateIf {
                    condition: IntermediateArg {
                        register: Register::new(),
                        type_: AtomicTypeEnum::BOOL.into(),
                    }
                    .into(),
                    branches: (
                        (
                            vec![IntermediateAssignment {
                                register: target_0.register.clone(),
                                expression: IntermediateIf {
                                    condition: IntermediateArg {
                                        register: Register::new(),
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
                                register: target_1.register.clone(),
                                expression: IntermediateIf {
                                    condition: IntermediateArg {
                                        register: Register::new(),
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
        let mut redundancy_eliminator = RedundancyEliminator::new();
        let optimized_lambda = redundancy_eliminator.eliminate_from_lambda(IntermediateLambda {
            args: Vec::new(),
            block: IntermediateBlock {
                statements,
                ret: target.clone().into(),
            },
        });
        let copy_propagator = CopyPropagator::from_statements(&optimized_lambda.block.statements);
        let optimized_block = copy_propagator.propagate_copies_in_block(optimized_lambda.block);
        let optimized_statements = optimized_block.statements;
        assert_eq!(optimized_statements.len(), 1);
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IntermediateIf(IntermediateIf {
                    condition: _,
                    branches,
                }),
            register: _,
        }) = optimized_statements[0].clone()
        else {
            panic!()
        };
        assert_eq!(branches.0.statements.len(), 1);
        assert_eq!(branches.1.statements.len(), 1);
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IntermediateIf(IntermediateIf {
                    condition: _,
                    branches: true_branches,
                }),
            register: _,
        }) = branches.0.statements[0].clone()
        else {
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression:
                IntermediateExpression::IntermediateIf(IntermediateIf {
                    condition: _,
                    branches: false_branches,
                }),
            register: _,
        }) = branches.1.statements[0].clone()
        else {
            dbg!(branches.1.statements);
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression: IntermediateExpression::IntermediateLambda(lambda_0),
            register: register_0,
        }) = true_branches.1.statements[0].clone()
        else {
            dbg!(true_branches.1.statements);
            panic!()
        };
        let IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            expression: IntermediateExpression::IntermediateLambda(lambda_1),
            register: register_1,
        }) = false_branches.1.statements[0].clone()
        else {
            dbg!(false_branches.1.statements);
            panic!()
        };
        assert_ne!(register_0, register_1);
        assert_ne!(lambda_0.args, lambda_1.args);
    }

    #[test_case(
        {
            let expression = IntermediateTupleExpression(Vec::new());
            let assignment = IntermediateAssignment{
                expression: expression.clone().into(),
                register: Register::new()
            };
            let ret = IntermediateAssignment{
                expression: expression.clone().into(),
                register: Register::new()
            };
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![None, None]).into()))
            ];
            (
                IntermediateProgram{
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock {
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
                    main: IntermediateLambda{
                        args: Vec::new(),
                        block: IntermediateBlock {
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
        let optimized_program = RedundancyEliminator::eliminate_redundancy(program);
        dbg!(&optimized_program);
        dbg!(&expected_program);
        assert_eq!(optimized_program.types, expected_program.types);
        ExpressionEqualityChecker::assert_equal(
            &optimized_program.main.into(),
            &expected_program.main.into(),
        )
    }
}
