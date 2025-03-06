use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateCtorCall, IntermediateElementAccess,
    IntermediateExpression, IntermediateFnCall, IntermediateIfStatement, IntermediateLambda,
    IntermediateMatchStatement, IntermediateMemory, IntermediateStatement,
    IntermediateTupleExpression, IntermediateValue, Location,
};
use std::collections::HashMap;

pub struct Refresher {
    locations: HashMap<Location, Location>,
}

impl Refresher {
    pub fn new() -> Self {
        Refresher {
            locations: HashMap::new(),
        }
    }
    pub fn refresh(lambda: &mut IntermediateLambda) {
        Refresher::new().refresh_lambda(lambda);
    }
    pub fn register_statements(&mut self, statements: &Vec<IntermediateStatement>) {
        let targets = IntermediateStatement::all_targets(statements);
        for target in targets {
            self.locations.insert(target, Location::new());
        }
    }
    fn refresh_lambda(&mut self, lambda: &mut IntermediateLambda) {
        self.register_statements(&lambda.statements);
        for arg in &mut lambda.args {
            self.refresh_arg(arg);
        }
        self.refresh_statements(&mut lambda.statements);
        self.refresh_value(&mut lambda.ret);
    }
    pub fn refresh_statements(&mut self, statements: &mut Vec<IntermediateStatement>) {
        for statement in statements.iter_mut() {
            self.refresh_statement(statement);
        }
    }
    fn refresh_statement(&mut self, statement: &mut IntermediateStatement) {
        match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                self.refresh_location(location);
                self.refresh_expression(expression)
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                self.refresh_value(condition);
                self.refresh_statements(&mut branches.0);
                self.refresh_statements(&mut branches.1);
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => {
                self.refresh_value(subject);
                for branch in branches {
                    if let Some(arg) = &mut branch.target {
                        self.refresh_arg(arg);
                    }
                    self.refresh_statements(&mut branch.statements);
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
            IntermediateExpression::IntermediateLambda(lambda) => self.refresh_lambda(lambda),
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
            IntermediateValue::IntermediateMemory(IntermediateMemory { type_: _, location }) => {
                self.refresh_location(location);
            }
            IntermediateValue::IntermediateArg(IntermediateArg { type_, location }) => {
                if self.refresh_location(location) {
                    *value = IntermediateMemory {
                        type_: type_.clone(),
                        location: location.clone(),
                    }
                    .into()
                }
            }
        }
    }
    fn refresh_location(&mut self, location: &mut Location) -> bool {
        if let Some(updated_location) = self.locations.get(location) {
            *location = updated_location.clone();
            true
        } else {
            false
        }
    }
    fn refresh_arg(&mut self, arg: &mut IntermediateArg) {
        let location = Location::new();
        self.locations
            .insert(arg.location.clone(), location.clone());
        arg.location = location;
    }
}
