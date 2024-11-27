use serde::{Deserialize, Serialize};

type Id = String;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AtomicTypeEnum {
    INT,
    BOOL,
}

#[derive(Serialize, Deserialize, Debug)]
struct AtomicType {
    type_: AtomicTypeEnum,
}

#[derive(Serialize, Deserialize, Debug)]
struct GenericType {
    id: Id,
    type_variables: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
struct TupleType {
    types: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
struct FunctionType {
    argument_type: Box<TypeInstance>,
    return_type: Box<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TypeInstance {
    Function(FunctionType),
    Atomic(AtomicType),
    Tuple(TupleType),
    Generic(GenericType),
}

#[cfg(test)]
mod tests {

    use super::*;

    use test_case::test_case;

    #[test_case(
        "\"INT\"",
        AtomicTypeEnum::INT;
        "atomic type enum int"
    )]
    #[test_case(
        "\"BOOL\"",
        AtomicTypeEnum::BOOL;
        "atomic type enum bool"
    )]
    fn test_deserialize_json<
        T: std::fmt::Debug + std::cmp::PartialEq + for<'a> serde::Deserialize<'a> + serde::Serialize,
    >(
        json: &str,
        node: T,
    ) {
        let result = serde_json::from_str::<T>(&json);
        if !result.is_ok() {
            println!("{:?}", serde_json::to_string(&node))
        }
        assert!(result.is_ok());
        let _ = result.inspect(|ast| assert_eq!(ast, &node));
    }
}
