use crate::{
    AtomicType, AtomicTypeEnum, Definition, Id, OpaqueTypeDefinition, TypeInstance,
    UnionTypeDefinition,
};
use std::{collections::HashMap, fmt::format};

struct TypeChecker {}

#[derive(Debug, PartialEq)]
enum Type {
    Atomic(AtomicTypeEnum),
    Union(Vec<Variant>),
}

#[derive(Debug, PartialEq)]
struct Variant {
    name: Id,
    type_: Option<Type>,
}

type TypeDefinitions = HashMap<String, Type>;

impl TypeChecker {
    fn convert_ast_type(type_instance: TypeInstance) -> Type {
        match type_instance {
            TypeInstance::AtomicType(AtomicType {
                type_: atomic_type_enum,
            }) => Type::Atomic(atomic_type_enum),
            _ => panic!(),
        }
    }
    fn check_type_definitions(definitions: Vec<Definition>) -> Result<TypeDefinitions, String> {
        let mut type_definitions: TypeDefinitions = HashMap::new();
        for definition in definitions {
            match definition {
                Definition::OpaqueTypeDefinition(OpaqueTypeDefinition { variable, type_ }) => {
                    if type_definitions.contains_key(&variable.id) {
                        return Err(format!("Duplicate type definition '{}'", variable.id));
                    }
                    type_definitions.insert(variable.id, TypeChecker::convert_ast_type(type_));
                }
                Definition::UnionTypeDefinition(UnionTypeDefinition { variable, items }) => {
                    if type_definitions.contains_key(&variable.id) {
                        return Err(format!("Duplicate type definition '{}'", variable.id));
                    }
                    let variants = items.into_iter().map(|item| Variant {
                        name: item.id,
                        type_: item.type_.map(TypeChecker::convert_ast_type),
                    });
                    type_definitions.insert(variable.id, Type::Union(variants.collect()));
                }
                _ => panic!(),
            }
        }
        return Ok(type_definitions);
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        GenericTypeVariable, TypeItem, TypeVariable, UnionTypeDefinition, ATOMIC_TYPE_BOOL,
        ATOMIC_TYPE_INT,
    };

    use super::*;

    use test_case::test_case;

    #[test_case(
        Vec::new(),
        Some(HashMap::new());
        "empty definitions"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_INT.into()
            }.into()
        ],
        Some(HashMap::from([
            (Id::from("i"), Type::Atomic(AtomicTypeEnum::INT))
        ]));
        "atomic opaque type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_BOOL.into()
            }.into()
        ],
        None;
        "duplicate opaque type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_INT.into()
            }.into()
        ],
        None;
        "duplicate opaque type name"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: TypeVariable("int_or_bool"),
                items: vec![
                    TypeItem{
                        id: Id::from("Int"),
                        type_: Some(ATOMIC_TYPE_INT.into())
                    },
                    TypeItem{
                        id: Id::from("Bool"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                ]
            }.into()
        ],
        Some(HashMap::from([
            (
                Id::from("int_or_bool"),
                Type::Union(
                    vec![
                        Variant {
                            name: Id::from("Int"),
                            type_: Some(Type::Atomic(AtomicTypeEnum::INT))
                        },
                        Variant {
                            name: Id::from("Bool"),
                            type_: Some(Type::Atomic(AtomicTypeEnum::BOOL))
                        },
                    ]
                )
            )
        ]));
        "basic union type definition"
    )]
    fn test_check_type_definitions(
        definitions: Vec<Definition>,
        expected_result: Option<TypeDefinitions>,
    ) {
        let type_check_result = TypeChecker::check_type_definitions(definitions);
        match expected_result {
            Some(type_definitions) => {
                assert_eq!(type_check_result, Ok(type_definitions))
            }
            None => {
                assert!(type_check_result.is_err())
            }
        }
    }
}
