use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateCtorCall, IntermediateElementAccess,
    IntermediateExpression, IntermediateFnCall, IntermediateIfStatement, IntermediateLambda,
    IntermediateMatchStatement, IntermediateMemory, IntermediateStatement,
    IntermediateTupleExpression, IntermediateValue, Location,
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Refresher {
    locations: HashMap<Location, IntermediateValue>,
}

impl Refresher {
    pub fn new() -> Self {
        Refresher {
            locations: HashMap::new(),
        }
    }
    pub fn refresh_for_inlining(lambda: &mut IntermediateLambda) {
        let mut refresher = Refresher::new();
        for arg in lambda.args.iter_mut() {
            let IntermediateArg { type_, location } = arg.clone();
            let memory = IntermediateMemory::from(type_);
            refresher.locations.insert(location, memory.clone().into());
            arg.location = memory.location;
        }
        refresher.refresh_statements(&mut lambda.statements);
        refresher.refresh_value(&mut lambda.ret);
    }
    pub fn refresh(lambda: &mut IntermediateLambda) {
        Refresher::new().refresh_lambda(lambda);
    }
    pub fn register_statements(&mut self, statements: &Vec<IntermediateStatement>) {
        let targets = statements.iter().filter_map(|statement| {
            if let IntermediateStatement::IntermediateAssignment(assignment) = statement {
                Some(assignment.clone())
            } else {
                None
            }
        });
        for target in targets {
            self.locations.entry(target.location).or_insert(
                IntermediateMemory {
                    location: Location::new(),
                    type_: target.expression.type_(),
                }
                .into(),
            );
        }
    }
    fn refresh_lambda(mut self, lambda: &mut IntermediateLambda) {
        for arg in &mut lambda.args {
            self.refresh_arg(arg);
        }
        self.refresh_statements(&mut lambda.statements);
        self.refresh_value(&mut lambda.ret);
    }
    fn refresh_statements(
        &mut self,
        statements: &mut Vec<IntermediateStatement>,
    ) -> Option<(Location, IntermediateValue)> {
        self.register_statements(statements);
        let mut it = statements.iter_mut().peekable();
        while let Some(statement) = it.next() {
            let last = self.refresh_statement(statement);
            if it.peek().is_none() {
                return last;
            }
        }
        None
    }
    fn refresh_branch_statements(
        &mut self,
        statements: &mut Vec<IntermediateStatement>,
    ) -> Option<(Location, IntermediateValue)> {
        let last = self.clone().refresh_statements(statements);
        if let Some((k, v)) = last.clone() {
            self.locations.insert(k, v);
        }
        last
    }
    fn refresh_statement(
        &mut self,
        statement: &mut IntermediateStatement,
    ) -> Option<(Location, IntermediateValue)> {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                self.refresh_expression(expression);
                if let Some(IntermediateValue::IntermediateMemory(memory)) =
                    self.refresh_location(location)
                {
                    let previous_location = location.clone();
                    *location = memory.location.clone();
                    Some((previous_location, memory.into()))
                } else {
                    None
                }
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                self.refresh_value(condition);
                self.refresh_branch_statements(&mut branches.1);
                self.refresh_branch_statements(&mut branches.0)
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => {
                self.refresh_value(subject);
                let mut it = branches.iter_mut().peekable();
                while let Some(branch) = it.next() {
                    if let Some(arg) = &mut branch.target {
                        self.refresh_arg(arg);
                    }
                    let last = self.refresh_branch_statements(&mut branch.statements);
                    if it.peek().is_none() {
                        return last;
                    }
                }
                None
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
                self.clone().refresh_lambda(lambda)
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
            IntermediateValue::IntermediateMemory(IntermediateMemory { type_: _, location })
            | IntermediateValue::IntermediateArg(IntermediateArg { type_: _, location }) => {
                if let Some(updated_value) = self.refresh_location(location) {
                    *value = updated_value;
                }
            }
        }
    }
    fn refresh_location(&mut self, location: &mut Location) -> Option<IntermediateValue> {
        self.locations.get(location).cloned()
    }
    fn refresh_arg(&mut self, arg: &mut IntermediateArg) {
        let location = Location::new();
        self.locations.insert(
            arg.location.clone(),
            IntermediateArg {
                location: location.clone(),
                type_: arg.type_.clone(),
            }
            .into(),
        );
        arg.location = location;
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
                    location: Location::new(),
                },
                IntermediateArg{
                    type_: AtomicTypeEnum::INT.into(),
                    location: Location::new(),
                },
            ];
            let ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
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
                statements: vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![x.clone()],
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
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: vec![y.clone()],
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
                        }.into(),
                        location: bar.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        location: main_call.location.clone(),
                        expression: IntermediateFnCall{
                            fn_: foo.clone().into(),
                            args: vec![z.clone().into()]
                        }.into()
                    }.into()
                ],
                ret: main_call.clone().into()
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
