use crate::{
    AtomicType, AtomicTypeEnum, Definition, GenericType, Id, OpaqueTypeDefinition, TypeInstance,
    UnionTypeDefinition,
};
use counter::Counter;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

struct TypeChecker {}

#[derive(PartialEq)]
enum Type {
    Atomic(AtomicTypeEnum),
    Union(Vec<Variant>),
    Reference(Rc<RefCell<Self>>),
    Empty,
}

const TYPE_INT: Type = Type::Atomic(AtomicTypeEnum::INT);
const TYPE_BOOL: Type = Type::Atomic(AtomicTypeEnum::BOOL);

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Atomic(atomic_type) => write!(f, "Atomic({:?})", atomic_type),
            Type::Union(variants) => write!(f, "Union({:?})", variants),
            Type::Reference(reference) => {
                // Include information about the memory address for clarity
                write!(f, "Reference({:p})", Rc::as_ptr(reference),)
            }
            Type::Empty => write!(f, "Empty"),
        }
    }
}

#[derive(Debug, PartialEq)]
struct Variant {
    id: Id,
    type_: Option<Type>,
}

type TypeReferencesIndex = HashMap<*mut Type, Id>;

struct TypeDefinitions(HashMap<Id, Rc<RefCell<Type>>>);

impl TypeDefinitions {
    pub fn new() -> Self {
        TypeDefinitions(HashMap::new())
    }
    pub fn get(&self, k: &Id) -> Option<&Rc<RefCell<Type>>> {
        self.0.get(k)
    }
    pub fn get_mut(&mut self, k: &Id) -> Option<&mut Rc<RefCell<Type>>> {
        self.0.get_mut(k)
    }
    fn references_index(&self) -> TypeReferencesIndex {
        self.0
            .iter()
            .map(|(key, value)| (value.clone().as_ptr(), key.clone()))
            .collect::<HashMap<_, _>>()
    }
    fn type_equality(
        self_references_index: &TypeReferencesIndex,
        other_references_index: &TypeReferencesIndex,
        t1: &Type,
        t2: &Type,
    ) -> bool {
        match (t1, t2) {
            (Type::Empty, Type::Empty) => true,
            (Type::Atomic(a1), Type::Atomic(a2)) => a1 == a2,
            (Type::Union(v1), Type::Union(v2)) => v1.iter().zip(v2.iter()).all(|(i1, i2)| {
                i1.id == i2.id
                    && match (&i1.type_, &i2.type_) {
                        (None, None) => true,
                        (Some(t1), Some(t2)) => TypeDefinitions::type_equality(
                            self_references_index,
                            other_references_index,
                            t1,
                            t2,
                        ),
                        _ => false,
                    }
            }),
            (Type::Reference(t1), Type::Reference(t2)) => {
                self_references_index.get(&t1.as_ptr()) == other_references_index.get(&t2.as_ptr())
            }
            _ => false,
        }
    }
}

impl From<HashMap<Id, Rc<RefCell<Type>>>> for TypeDefinitions {
    fn from(value: HashMap<Id, Rc<RefCell<Type>>>) -> Self {
        return TypeDefinitions(value);
    }
}

impl<const N: usize> From<[(Id, Type); N]> for TypeDefinitions {
    fn from(arr: [(Id, Type); N]) -> Self {
        arr.into_iter().collect()
    }
}

impl<const N: usize> From<[(Id, Rc<RefCell<Type>>); N]> for TypeDefinitions {
    fn from(arr: [(Id, Rc<RefCell<Type>>); N]) -> Self {
        TypeDefinitions(HashMap::from(arr))
    }
}

impl FromIterator<(Id, Type)> for TypeDefinitions {
    fn from_iter<T: IntoIterator<Item = (Id, Type)>>(iter: T) -> Self {
        TypeDefinitions(
            iter.into_iter()
                .map(|(id, type_)| (id, Rc::new(RefCell::new(type_))))
                .collect(),
        )
    }
}

impl FromIterator<(Id, Rc<RefCell<Type>>)> for TypeDefinitions {
    fn from_iter<T: IntoIterator<Item = (Id, Rc<RefCell<Type>>)>>(iter: T) -> Self {
        TypeDefinitions(HashMap::from_iter(iter))
    }
}

impl fmt::Debug for TypeDefinitions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let references_index = Box::new(self.references_index());
        f.debug_map()
            .entries(self.0.iter().map(|(key, value)| {
                (
                    key,
                    DebugTypeWrapper(value.clone(), references_index.clone()),
                )
            }))
            .finish()
    }
}

struct DebugTypeWrapper(Rc<RefCell<Type>>, Box<HashMap<*mut Type, Id>>);
impl fmt::Debug for DebugTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let references_index = &self.1;
        match &*self.0.borrow() {
            Type::Atomic(atomic_type_enum) => write!(f, "Atomic({:?})", atomic_type_enum),
            Type::Union(vec) => write!(f, "Union({:?})", vec),
            Type::Reference(rc) => write!(
                f,
                "{:?}",
                references_index
                    .get(&rc.as_ptr())
                    .unwrap_or(&Id::from("unknown"))
            ),
            Type::Empty => write!(f, "Empty"),
        }
    }
}

impl PartialEq for TypeDefinitions {
    fn eq(&self, other: &Self) -> bool {
        if self.0.keys().collect::<HashSet<_>>() != other.0.keys().collect::<HashSet<_>>() {
            return false;
        }
        let self_references_index = &self.references_index();
        let other_references_index = &other.references_index();
        self.0
            .keys()
            .map(|key| (self.0.get(key), other.0.get(key)))
            .all(|(v1, v2)| match (v1, v2) {
                (Some(t1), Some(t2)) => TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    &*t1.borrow(),
                    &*t2.borrow(),
                ),
                _ => false,
            })
    }
}

impl TypeChecker {
    fn convert_ast_type(type_instance: &TypeInstance, type_definitions: &TypeDefinitions) -> Type {
        match type_instance {
            TypeInstance::AtomicType(AtomicType {
                type_: atomic_type_enum,
            }) => Type::Atomic(atomic_type_enum.clone()),
            TypeInstance::GenericType(GenericType { id, type_variables }) => {
                if let Some(reference) = type_definitions.get(id) {
                    Type::Reference(reference.clone())
                } else {
                    panic!("{} not found in type definitions", id)
                }
            }
            _ => todo!(),
        }
    }
    fn check_type_definitions(definitions: &Vec<Definition>) -> Result<TypeDefinitions, Id> {
        let type_names = definitions.iter().map(Definition::get_name);
        if !type_names.clone().all_unique() {
            let type_name_counts = type_names.collect::<Counter<_>>();
            for (name, count) in type_name_counts {
                if count > 1 {
                    return Err(format!("Duplicated type name {}", name));
                }
            }
            panic!("Type names were not unique but all counts were < 2");
        }
        let mut type_definitions: TypeDefinitions = type_names
            .map(|name| (name.clone(), Type::Empty))
            .collect::<TypeDefinitions>();
        for definition in definitions {
            let type_name = definition.get_name();
            let type_ = match &definition {
                Definition::OpaqueTypeDefinition(OpaqueTypeDefinition { variable: _, type_ }) => {
                    TypeChecker::convert_ast_type(type_, &type_definitions)
                }
                Definition::UnionTypeDefinition(UnionTypeDefinition { variable: _, items }) => {
                    let variants = items.iter().map(|item| Variant {
                        id: item.id.clone(),
                        type_: item.type_.as_ref().map(|type_instance| {
                            TypeChecker::convert_ast_type(type_instance, &type_definitions)
                        }),
                    });
                    Type::Union(variants.collect())
                }
                _ => todo!(),
            };
            if let Some(type_reference) = type_definitions.get_mut(type_name) {
                *type_reference.borrow_mut() = type_;
            } else {
                panic!("{} not found in type definitions", type_name)
            }
        }
        return Ok(type_definitions);
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        TypeItem, TypeVariable, Typename, UnionTypeDefinition, ATOMIC_TYPE_BOOL, ATOMIC_TYPE_INT,
    };

    use super::*;

    use test_case::test_case;

    #[test_case(
        Vec::new(),
        Some(TypeDefinitions::new());
        "empty definitions"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("i"),
                type_: ATOMIC_TYPE_INT.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (Id::from("i"), Rc::new(RefCell::new(TYPE_INT)))
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
        Some(TypeDefinitions::from([
            (
                Id::from("int_or_bool"),
                Type::Union(
                    vec![
                        Variant {
                            id: Id::from("Int"),
                            type_: Some(TYPE_INT)
                        },
                        Variant {
                            id: Id::from("Bool"),
                            type_: Some(TYPE_BOOL)
                        },
                    ]
                )
            )
        ]));
        "basic union type definition"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: TypeVariable("int_list"),
                items: vec![
                    TypeItem{
                        id: Id::from("Cons"),
                        type_: Some(Typename("int_list").into())
                    },
                    TypeItem{
                        id: Id::from("Nil"),
                        type_: None
                    },
                ]
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("int_list"),
                ({
                    let reference = Rc::new(RefCell::new(Type::Empty));
                    let union_type = Type::Union(vec![
                        Variant {
                            id: Id::from("Cons"),
                            type_: Some(Type::Reference(Rc::clone(&reference))),
                        },
                        Variant {
                            id: Id::from("Nil"),
                            type_: None,
                        },
                    ]);
                    *reference.borrow_mut() = union_type;
                    reference
                })
            )
        ]));
        "recursive type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("int"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: TypeVariable("bool"),
                type_: ATOMIC_TYPE_BOOL.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (Id::from("int"), TYPE_INT),
            (Id::from("bool"), TYPE_BOOL),
        ]));
        "two type definitions"
    )]
    fn test_check_type_definitions(
        definitions: Vec<Definition>,
        expected_result: Option<TypeDefinitions>,
    ) {
        let type_check_result = TypeChecker::check_type_definitions(&definitions);
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
