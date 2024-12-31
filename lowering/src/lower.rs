use std::collections::HashMap;

use from_variants::FromVariants;
use type_checker::*;

#[derive(FromVariants, Debug, Clone, PartialEq)]
enum Value {
    Register(Register),
    BuiltIn(BuiltIn),
}

#[derive(FromVariants, Debug, Clone, PartialEq)]
enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    Function(Function),
}

#[derive(Debug, Clone, PartialEq)]
struct Function {
    name: String,
    return_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
struct Register {}

type Scope = HashMap<Variable, Value>;
type History = HashMap<Value, Register>;

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

    #[test]
    fn test_lower_int_expressions() {
        let expression = TypedExpression::Integer(Integer { value: 4 });
        let scope = Scope::new();
        let mut history = History::new();
        let computation = Lowerer::lower_computations(expression, &scope, &mut history);
        assert_eq!(
            computation,
            Value::BuiltIn(BuiltIn::Integer(Integer { value: 4 }))
        )
    }
    #[test]
    fn test_lower_bool_expressions() {
        let expression = TypedExpression::Boolean(Boolean { value: true });
        let scope = Scope::new();
        let mut history = History::new();
        let computation = Lowerer::lower_computations(expression, &scope, &mut history);
        assert_eq!(
            computation,
            Value::BuiltIn(BuiltIn::Boolean(Boolean { value: true }))
        )
    }
}
