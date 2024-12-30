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
    fn extract_computations(
        expression: TypedExpression,
        scope: &Scope,
        history: &mut History,
    ) -> Value {
        BuiltIn::from(Integer { value: 4 }).into()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_extract_int_expressions() {
        let expression = TypedExpression::Integer(Integer { value: 4 });
        let scope = Scope::new();
        let mut history = History::new();
        let computation = Lowerer::extract_computations(expression, &scope, &mut history);
        assert_eq!(
            computation,
            Value::BuiltIn(BuiltIn::Integer(Integer { value: 4 }))
        )
    }
}
