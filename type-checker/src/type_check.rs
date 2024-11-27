use crate::Definition;
use std::collections::HashMap;

struct TypeChecker {}

#[derive(Debug, PartialEq)]
enum Type {}

type TypeDefinitions = HashMap<String, Type>;

impl TypeChecker {
    fn check_type_definitions(definitions: Vec<Definition>) -> Result<TypeDefinitions, String> {
        return Ok(HashMap::new());
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use test_case::test_case;

    #[test_case(
        Vec::new(),
        Ok(HashMap::new());
        "empty definitions"
    )]
    fn test_check_type_definitions(
        definitions: Vec<Definition>,
        result: Result<TypeDefinitions, String>,
    ) {
        assert_eq!(TypeChecker::check_type_definitions(definitions), result);
    }
}
