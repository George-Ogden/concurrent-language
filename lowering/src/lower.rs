use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::intermediate_nodes::*;
use type_checker::*;

struct Lowerer {
    scope: HashMap<Variable, IntermediateValue>,
    history: HashMap<IntermediateExpression, IntermediateValue>,
    uninstantiated: HashMap<(Variable, Rc<RefCell<Option<Type>>>), IntermediateExpression>,
    type_defs: HashMap<Type, Rc<RefCell<Vec<Option<IntermediateType>>>>>,
    statements: Vec<IntermediateStatement>,
}
impl Lowerer {
    pub fn new() -> Self {
        Lowerer {
            scope: HashMap::new(),
            history: HashMap::new(),
            uninstantiated: HashMap::new(),
            type_defs: HashMap::new(),
            statements: Vec::new(),
        }
    }
    fn lower_expression(&mut self, expression: TypedExpression) -> IntermediateValue {
        match expression {
            TypedExpression::Integer(integer) => IntermediateBuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => IntermediateBuiltIn::Boolean(boolean).into(),
            TypedExpression::TypedTuple(TypedTuple { expressions }) => {
                let intermediate_expressions = self.lower_expressions(expressions);
                let intermediate_expression: IntermediateExpression =
                    IntermediateTupleExpression(intermediate_expressions).into();
                self.history
                    .entry(intermediate_expression.clone())
                    .or_insert(intermediate_expression.into())
                    .clone()
            }
            _ => todo!(),
        }
    }
    fn lower_expressions(&mut self, expressions: Vec<TypedExpression>) -> Vec<IntermediateValue> {
        expressions
            .into_iter()
            .map(|expression| self.lower_expression(expression))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use test_case::test_case;

    #[test_case(
        TypedExpression::Integer(Integer { value: 4 }),
        IntermediateBuiltIn::Integer(Integer { value: 4 }).into();
        "integer"
    )]
    #[test_case(
        TypedExpression::Boolean(Boolean { value: true }),
        IntermediateBuiltIn::Boolean(Boolean { value: true }).into();
        "boolean"
    )]
    #[test_case(
        TypedTuple{
            expressions: Vec::new()
        }.into(),
        IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new())).into();
        "empty tuple"
    )]
    #[test_case(
        TypedTuple{
            expressions: vec![
                Integer{value: 3}.into(),
                Boolean{value: false}.into()
            ]
        }.into(),
        IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
            vec![
                IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
            ]
        )).into();
        "non-empty tuple"
    )]
    #[test_case(
        TypedTuple{
            expressions: vec![
                TypedTuple{
                    expressions: Vec::new()
                }.into(),
                Integer{value: 1}.into(),
                TypedTuple{
                    expressions: vec![
                        Boolean{value: true}.into()
                    ]
                }.into(),
            ]
        }.into(),
        IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
            vec![
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new()).into()).into(),
                IntermediateBuiltIn::Integer(Integer { value: 1 }).into(),
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                    vec![
                        IntermediateBuiltIn::Boolean(Boolean { value: true }).into(),
                    ]
                )).into()
            ]
        )).into();
        "nested tuple"
    )]
    fn test_lower_expression(expression: TypedExpression, value: IntermediateValue) {
        let mut lowerer = Lowerer::new();
        let computation = lowerer.lower_expression(expression);
        assert_eq!(computation, value)
    }
}
