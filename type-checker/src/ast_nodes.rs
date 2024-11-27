use serde::{Deserialize, Serialize};

type Id = String;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum AtomicTypeEnum {
    INT,
    BOOL,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AtomicType {
    type_: AtomicTypeEnum,
}

const ATOMIC_TYPE_INT: AtomicType = AtomicType {
    type_: AtomicTypeEnum::INT,
};
const ATOMIC_TYPE_BOOL: AtomicType = AtomicType {
    type_: AtomicTypeEnum::BOOL,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct GenericType {
    id: Id,
    type_variables: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TupleType {
    types: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct FunctionType {
    argument_type: Box<TypeInstance>,
    return_type: Box<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum TypeInstance {
    FunctionType(FunctionType),
    AtomicType(AtomicType),
    TupleType(TupleType),
    GenericType(GenericType),
}

#[cfg(test)]
mod tests {

    use super::*;

    use test_case::test_case;

    #[test_case(
        r#""INT""#,
        AtomicTypeEnum::INT;
        "atomic type enum int"
    )]
    #[test_case(
        r#""BOOL""#,
        AtomicTypeEnum::BOOL;
        "atomic type enum bool"
    )]
    #[test_case(
        r#"{"type_": "BOOL"}"#,
        AtomicType{type_: AtomicTypeEnum::BOOL};
        "atomic type bool"
    )]
    #[test_case(
        r#"{"types": []}"#,
        TupleType{types: Vec::new()};
        "empty tuple type"
    )]
    #[test_case(
        r#"{"types":[{"AtomicType":{"type_":"BOOL"}},{"TupleType":{"types":[]}}]}"#,
        TupleType{
            types: vec![
                TypeInstance::AtomicType(ATOMIC_TYPE_BOOL),
                TypeInstance::TupleType(TupleType{types: Vec::new()}),
            ]
        };
        "non-empty tuple type"
    )]
    #[test_case(
        r#"{"argument_type":{"TupleType":{"types":[{"AtomicType":{"type_":"INT"}}]}},"return_type":{"AtomicType":{"type_":"INT"}}}"#,
        FunctionType{
            argument_type: Box::new(
                TypeInstance::TupleType(TupleType{
                    types: vec![TypeInstance::AtomicType(ATOMIC_TYPE_INT)]
                })
            ),
            return_type: Box::new(
                TypeInstance::AtomicType(ATOMIC_TYPE_INT)
            )
        };
        "function type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"AtomicType":{"type_":"INT"}},{"AtomicType":{"type_":"BOOL"}}]}"#,
        GenericType{
            id: String::from("map"),
            type_variables: vec![
                TypeInstance::AtomicType(ATOMIC_TYPE_INT),
                TypeInstance::AtomicType(ATOMIC_TYPE_BOOL)
            ]
        };
        "generic type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"FunctionType":{"argument_type":{"AtomicType":{"type_":"INT"}},"return_type":{"AtomicType":{"type_":"INT"}}}},{"GenericType":{"id":"foo","type_variables":[]}}]}"#,
        GenericType{
            id: String::from("map"),
            type_variables: vec![
                TypeInstance::FunctionType(FunctionType{
                    argument_type: Box::new(
                        TypeInstance::AtomicType(ATOMIC_TYPE_INT)
                    ),
                    return_type: Box::new(
                        TypeInstance::AtomicType(ATOMIC_TYPE_INT)
                    )
                }),
                TypeInstance::GenericType(GenericType {
                    id: String::from("foo"),
                    type_variables: Vec::new()
                })
            ]
        };
        "nested generic type"
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
