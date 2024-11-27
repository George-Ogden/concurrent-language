use serde::{Deserialize, Serialize};
use std::convert::From;

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

impl From<FunctionType> for TypeInstance {
    fn from(value: FunctionType) -> Self {
        TypeInstance::FunctionType(value)
    }
}

impl From<AtomicType> for TypeInstance {
    fn from(value: AtomicType) -> Self {
        TypeInstance::AtomicType(value)
    }
}

impl From<TupleType> for TypeInstance {
    fn from(value: TupleType) -> Self {
        TypeInstance::TupleType(value)
    }
}

impl From<GenericType> for TypeInstance {
    fn from(value: GenericType) -> Self {
        TypeInstance::GenericType(value)
    }
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
                ATOMIC_TYPE_BOOL.into(),
                TupleType{types: Vec::new()}.into(),
            ]
        };
        "non-empty tuple type"
    )]
    #[test_case(
        r#"{"argument_type":{"TupleType":{"types":[{"AtomicType":{"type_":"INT"}}]}},"return_type":{"AtomicType":{"type_":"INT"}}}"#,
        FunctionType{
            argument_type: Box::new(
                TupleType{
                    types: vec![ATOMIC_TYPE_INT.into()]
                }.into()
            ),
            return_type: Box::new(
                ATOMIC_TYPE_INT.into()
            )
        };
        "function type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"AtomicType":{"type_":"INT"}},{"AtomicType":{"type_":"BOOL"}}]}"#,
        GenericType{
            id: String::from("map"),
            type_variables: vec![
                ATOMIC_TYPE_INT.into(),
                ATOMIC_TYPE_BOOL.into()
            ]
        };
        "generic type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"FunctionType":{"argument_type":{"AtomicType":{"type_":"INT"}},"return_type":{"AtomicType":{"type_":"INT"}}}},{"GenericType":{"id":"foo","type_variables":[]}}]}"#,
        GenericType{
            id: String::from("map"),
            type_variables: vec![
                FunctionType{
                    argument_type: Box::new(
                        ATOMIC_TYPE_INT.into()
                    ),
                    return_type: Box::new(
                        ATOMIC_TYPE_INT.into()
                    )
                }.into(),
                GenericType {
                    id: String::from("foo"),
                    type_variables: Vec::new()
                }.into()
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
