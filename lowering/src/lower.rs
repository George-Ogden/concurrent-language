use std::collections::HashMap;

use crate::{BuiltIn, Memory, Value};
use type_checker::*;

type Scope = HashMap<Variable, Value>;
type History = HashMap<Value, Memory>;

struct Lowerer {}
impl Lowerer {
    fn lower_computations(
        expression: TypedExpression,
        scope: &Scope,
        history: &mut History,
    ) -> Value {
        match expression {
            TypedExpression::Integer(integer) => BuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => BuiltIn::Boolean(boolean).into(),
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
        BuiltIn::Integer(Integer { value: 4 }).into(),
        Scope::new();
        "integer"
    )]
    #[test_case(
        TypedExpression::Boolean(Boolean { value: true }),
        BuiltIn::Boolean(Boolean { value: true }).into(),
        Scope::new();
        "boolean"
    )]
    fn test_lower_expression(expression: TypedExpression, value: Value, scope: Scope) {
        let mut history = History::new();
        let computation = Lowerer::lower_computations(expression, &scope, &mut history);
        assert_eq!(computation, value)
    }
}
