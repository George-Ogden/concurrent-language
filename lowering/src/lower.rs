use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::intermediate_nodes::*;
use type_checker::*;

type Scope = HashMap<Variable, IntermediateValue>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<(Variable, Rc<RefCell<Option<Type>>>), IntermediateExpression>;
type TypeDefs = HashMap<Type, Rc<RefCell<Vec<Option<IntermediateType>>>>>;

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
    fn lower_computations(&mut self, expression: TypedExpression) -> IntermediateValue {
        match expression {
            TypedExpression::Integer(integer) => IntermediateBuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => IntermediateBuiltIn::Boolean(boolean).into(),
            _ => todo!(),
        }
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
    fn test_lower_expression(expression: TypedExpression, value: IntermediateValue) {
        let mut lowerer = Lowerer::new();
        let computation = lowerer.lower_computations(expression);
        assert_eq!(computation, value)
    }
}
