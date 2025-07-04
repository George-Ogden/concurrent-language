use std::collections::HashMap;

use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateBlock, IntermediateCtorCall,
    IntermediateElementAccess, IntermediateExpression, IntermediateFnCall, IntermediateLambda,
    IntermediateMemory, IntermediateStatement, IntermediateTupleExpression, IntermediateValue,
    Register,
};

#[derive(Clone)]
pub struct Refresher {
    registers: HashMap<Register, IntermediateValue>,
}

/// Refresh potentially duplicated variable names.
impl Refresher {
    pub fn new() -> Self {
        Refresher {
            registers: HashMap::new(),
        }
    }
    /// Refresh arguments as memory so that they can be assigned to.
    pub fn refresh_for_inlining(lambda: &mut IntermediateLambda) {
        let mut refresher = Refresher::new();
        for arg in lambda.args.iter_mut() {
            let IntermediateArg { type_, register } = arg.clone();
            let memory = IntermediateMemory::from(type_);
            refresher.registers.insert(register, memory.clone().into());
            arg.register = memory.register;
        }
        refresher.refresh_block(&mut lambda.block);
    }
    pub fn refresh(lambda: &mut IntermediateLambda) {
        Refresher::new().refresh_lambda(lambda);
    }
    /// Store all assignment targets and allocate a new memory address.
    pub fn register_statements(&mut self, statements: &Vec<IntermediateStatement>) {
        let targets = statements.iter().filter_map(|statement| {
            let IntermediateStatement::IntermediateAssignment(assignment) = statement;
            Some((
                assignment.register.clone(),
                IntermediateMemory::from(assignment.expression.type_()).into(),
            ))
        });
        self.registers.extend(targets.clone());
    }
    fn refresh_lambda(mut self, lambda: &mut IntermediateLambda) {
        for arg in &mut lambda.args {
            self.refresh_arg(arg);
        }
        self.refresh_block(&mut lambda.block);
    }
    fn refresh_block(&mut self, block: &mut IntermediateBlock) {
        self.refresh_statements(&mut block.statements);
        self.refresh_value(&mut block.ret);
    }
    fn refresh_statements(&mut self, statements: &mut Vec<IntermediateStatement>) {
        self.register_statements(statements);
        for statement in statements {
            self.refresh_statement(statement);
        }
    }
    fn refresh_statement(&mut self, statement: &mut IntermediateStatement) {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                register,
            }) => {
                self.refresh_expression(expression);
                if let Some(IntermediateValue::IntermediateMemory(memory)) =
                    self.refresh_register(register)
                {
                    *register = memory.register.clone();
                }
            }
        }
    }
    fn refresh_expression(&mut self, expression: &mut IntermediateExpression) {
        match expression {
            IntermediateExpression::IntermediateValue(value) => {
                self.refresh_value(value);
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx: _,
            }) => {
                self.refresh_value(value);
            }
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                self.refresh_values(values);
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                self.refresh_value(fn_);
                self.refresh_values(args)
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx: _,
                data,
                type_: _,
            }) => match data {
                None => (),
                Some(data) => self.refresh_value(data),
            },
            IntermediateExpression::IntermediateLambda(lambda) => {
                // Refresh lambda independently.
                self.clone().refresh_lambda(lambda)
            }
            IntermediateExpression::IntermediateIf(if_) => {
                self.refresh_value(&mut if_.condition);
                // Refresh branches independently.
                self.clone().refresh_block(&mut if_.branches.0);
                self.clone().refresh_block(&mut if_.branches.1);
            }
            IntermediateExpression::IntermediateMatch(match_) => {
                self.refresh_value(&mut match_.subject);
                for branch in &mut match_.branches {
                    // Refresh branches independently.
                    let mut refresher = self.clone();
                    if let Some(arg) = &mut branch.target {
                        refresher.refresh_arg(arg);
                    }
                    refresher.refresh_block(&mut branch.block);
                }
            }
        }
    }
    fn refresh_values(&mut self, values: &mut Vec<IntermediateValue>) {
        for value in values {
            self.refresh_value(value)
        }
    }
    fn refresh_value(&mut self, value: &mut IntermediateValue) {
        match value {
            IntermediateValue::IntermediateBuiltIn(_) => {}
            IntermediateValue::IntermediateMemory(IntermediateMemory { type_: _, register })
            | IntermediateValue::IntermediateArg(IntermediateArg { type_: _, register }) => {
                if let Some(updated_value) = self.refresh_register(register) {
                    *value = updated_value;
                }
            }
        }
    }
    fn refresh_register(&mut self, register: &Register) -> Option<IntermediateValue> {
        self.registers.get(register).cloned()
    }
    fn refresh_arg(&mut self, arg: &mut IntermediateArg) {
        let register = Register::new();
        self.registers.insert(
            arg.register.clone(),
            IntermediateArg {
                type_: arg.type_.clone(),
                register: register.clone(),
            }
            .into(),
        );
        arg.register = register;
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use lowering::{
        AtomicTypeEnum, BuiltInFn, ExpressionEqualityChecker, Id, IntermediateBuiltIn,
        IntermediateFnType, IntermediateLambda, IntermediateType,
    };

    use super::*;
    use test_case::test_case;

    #[test_case(
        {
            let args = vec![
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    register: Register::new(),
                },
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    register: Register::new(),
                },
            ];
            let ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                register: Register::new()
            };
            IntermediateLambda {
                args: args.clone(),
                block: IntermediateBlock {
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
                            register: ret.register.clone()
                        }.into()
                    ],
                    ret: ret.clone().into()
                },
            }
        };
        "plus fn"
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
            let main_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            IntermediateLambda{
                args: vec![z.clone()],
                block: IntermediateBlock {
                    statements: vec![
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
                        IntermediateAssignment{
                            register: main_call.register.clone(),
                            expression: IntermediateFnCall{
                                fn_: foo.clone().into(),
                                args: vec![z.clone().into()]
                            }.into()
                        }.into()
                    ],
                    ret: main_call.clone().into()
                },
            }
        };
        "mutually recursive fns"
    )]
    fn test_refresh_lambda(lambda: IntermediateLambda) {
        let mut refreshed = lambda.clone();
        Refresher::refresh(&mut refreshed);
        dbg!(&lambda, &refreshed);
        ExpressionEqualityChecker::assert_equal(&refreshed.into(), &lambda.into());
    }
}
