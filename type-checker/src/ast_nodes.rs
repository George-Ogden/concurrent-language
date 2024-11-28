use serde::{Deserialize, Serialize};
use std::convert::From;

pub type Id = String;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum AtomicTypeEnum {
    INT,
    BOOL,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AtomicType {
    pub type_: AtomicTypeEnum,
}

pub const ATOMIC_TYPE_INT: AtomicType = AtomicType {
    type_: AtomicTypeEnum::INT,
};
pub const ATOMIC_TYPE_BOOL: AtomicType = AtomicType {
    type_: AtomicTypeEnum::BOOL,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GenericType {
    pub id: Id,
    pub type_variables: Vec<TypeInstance>,
}

pub fn Typename(name: &str) -> GenericType {
    GenericType {
        id: Id::from(name),
        type_variables: Vec::new(),
    }
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TypeItem {
    pub id: Id,
    pub type_: Option<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GenericTypeVariable {
    pub id: Id,
    generic_variables: Vec<Id>,
}

pub fn TypeVariable(name: &str) -> GenericTypeVariable {
    return GenericTypeVariable {
        id: String::from(name),
        generic_variables: Vec::new(),
    };
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct UnionTypeDefinition {
    pub variable: GenericTypeVariable,
    pub items: Vec<TypeItem>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct OpaqueTypeDefinition {
    pub variable: GenericTypeVariable,
    pub type_: TypeInstance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct EmptyTypeDefinition {
    id: Id,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TransparentTypeDefinition {
    variable: GenericTypeVariable,
    type_: TypeInstance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Definition {
    UnionTypeDefinition(UnionTypeDefinition),
    OpaqueTypeDefinition(OpaqueTypeDefinition),
    TransparentTypeDefinition(TransparentTypeDefinition),
    EmptyTypeDefinition(EmptyTypeDefinition),
}

impl Definition {
    pub fn get_name(&self) -> &Id {
        match self {
            Self::UnionTypeDefinition(UnionTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                items: _,
            })
            | Self::EmptyTypeDefinition(EmptyTypeDefinition { id })
            | Self::TransparentTypeDefinition(TransparentTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                type_: _,
            })
            | Self::OpaqueTypeDefinition(OpaqueTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                type_: _,
            }) => id,
        }
    }
}

impl From<OpaqueTypeDefinition> for Definition {
    fn from(value: OpaqueTypeDefinition) -> Self {
        Definition::OpaqueTypeDefinition(value)
    }
}

impl From<UnionTypeDefinition> for Definition {
    fn from(value: UnionTypeDefinition) -> Self {
        Definition::UnionTypeDefinition(value)
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
            id: Id::from("map"),
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
            id: Id::from("map"),
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
                    id: Id::from("foo"),
                    type_variables: Vec::new()
                }.into()
            ]
        };
        "nested generic type"
    )]
    #[test_case(
        r#"{"variable":{"id":"Maybe","generic_variables":["T"]},"items":[{"id":"Some","type_":{"GenericType":{"id":"T","type_variables":[]}}},{"id":"None","type_":null}]}"#,
        UnionTypeDefinition {
            variable: GenericTypeVariable{
                id: Id::from("Maybe"),
                generic_variables: vec![Id::from("T")]
            },
            items: vec![
                TypeItem {
                    id: Id::from("Some"),
                    type_: Some(Typename("T").into()),
                },
                TypeItem {
                    id: Id::from("None"),
                    type_: None
                }
            ]
        };
        "union type definition"
    )]
    #[test_case(
        r#"{"variable":{"id":"Pair","generic_variables":["T","U"]},"type_":{"TupleType":{"types":[{"GenericType":{"id":"T","type_variables":[]}},{"GenericType":{"id":"U","type_variables":[]}}]}}}"#,
        OpaqueTypeDefinition{
            variable: GenericTypeVariable{
                id: Id::from("Pair"),
                generic_variables: vec![Id::from("T"), Id::from("U")]
            },
            type_: TupleType{
                types: vec![Typename("T").into(), Typename("U").into()]
            }.into()
        };
        "opaque type definition"
    )]
    #[test_case(
        r#"{"id":"None"}"#,
        EmptyTypeDefinition{
            id: Id::from("None")
        };
        "empty type definition"
    )]
    #[test_case(
        r#"{"variable":{"id":"ii","generic_variables":[]},"type_":{"TupleType":{"types":[{"AtomicType":{"type_":"INT"}},{"AtomicType":{"type_":"INT"}}]}}}"#,
        TransparentTypeDefinition{
            variable: GenericTypeVariable{
                id: Id::from("ii"),
                generic_variables: Vec::new()
            },
            type_: TupleType{
                types: vec![
                    ATOMIC_TYPE_INT.into(),
                    ATOMIC_TYPE_INT.into(),
                ]
            }.into()
        };
        "transparent type definition"
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
