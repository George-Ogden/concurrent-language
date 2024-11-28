use crate::{
    AtomicType, AtomicTypeEnum, Definition, EmptyTypeDefinition, FunctionType, GenericType, Id,
    OpaqueTypeDefinition, TransparentTypeDefinition, TupleType, TypeInstance, UnionTypeDefinition,
};
use counter::Counter;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug};
use std::rc::Rc;
use strum::IntoEnumIterator;

struct TypeChecker {}

#[derive(PartialEq, Clone)]
enum Type {
    Atomic(AtomicTypeEnum),
    Union(Vec<Variant>),
    Reference(Rc<RefCell<Self>>),
    Tuple(Vec<Type>),
    Function(Box<Type>, Box<Type>),
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
                write!(f, "Reference({:p})", Rc::as_ptr(reference),)
            }
            Type::Empty => write!(f, "Empty"),
            Type::Tuple(types) => write!(f, "Tuple({:?})", types),
            Type::Function(argument_type, return_type) => {
                write!(f, "Function({:?},{:?})", argument_type, return_type)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
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
            (Type::Tuple(t1), Type::Tuple(t2)) => {
                t1.len() == t2.len()
                    && t1.iter().zip(t2.iter()).all(|(t1, t2)| {
                        TypeDefinitions::type_equality(
                            self_references_index,
                            other_references_index,
                            t1,
                            t2,
                        )
                    })
            }
            (Type::Function(a1, r1), Type::Function(a2, r2)) => {
                TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    a1,
                    a2,
                ) && TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    r1,
                    r2,
                )
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
                    DebugTypeWrapper(value.borrow().clone(), references_index.clone()),
                )
            }))
            .finish()
    }
}

struct DebugTypeWrapper(Type, Box<HashMap<*mut Type, Id>>);
impl fmt::Debug for DebugTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let references_index = &self.1;
        match self.0.clone() {
            Type::Atomic(atomic_type_enum) => write!(f, "Atomic({:?})", atomic_type_enum),
            Type::Union(variants) => {
                write!(
                    f,
                    "Union({:?})",
                    variants
                        .into_iter()
                        .map(|variant| {
                            (
                                variant.id,
                                variant
                                    .type_
                                    .map(|type_| DebugTypeWrapper(type_, references_index.clone())),
                            )
                        })
                        .collect_vec()
                )
            }
            Type::Reference(rc) => {
                write!(
                    f,
                    "Reference({})",
                    references_index
                        .get(&rc.as_ptr())
                        .unwrap_or(&Id::from("unknown"))
                )
            }
            Type::Empty => write!(f, "Empty"),
            Type::Tuple(types) => {
                write!(
                    f,
                    "Tuple({:?})",
                    types
                        .into_iter()
                        .map(|type_| DebugTypeWrapper(type_, references_index.clone()))
                        .collect_vec()
                )
            }
            Type::Function(argument_type, return_type) => {
                write!(
                    f,
                    "Function({:?},{:?})",
                    DebugTypeWrapper(*argument_type, references_index.clone()),
                    DebugTypeWrapper(*return_type, references_index.clone()),
                )
            }
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
            TypeInstance::TupleType(TupleType { types }) => Type::Tuple(
                types
                    .iter()
                    .map(|t| TypeChecker::convert_ast_type(t, type_definitions))
                    .collect_vec(),
            ),
            TypeInstance::FunctionType(FunctionType {
                argument_type,
                return_type,
            }) => Type::Function(
                Box::new(TypeChecker::convert_ast_type(
                    &argument_type,
                    type_definitions,
                )),
                Box::new(TypeChecker::convert_ast_type(
                    &return_type,
                    type_definitions,
                )),
            ),
        }
    }
    fn check_type_definitions(definitions: &Vec<Definition>) -> Result<TypeDefinitions, String> {
        let type_names = definitions.iter().map(Definition::get_name);
        let predefined_type_names = AtomicTypeEnum::iter()
            .map(|a| AtomicTypeEnum::to_string(&a).to_lowercase())
            .collect_vec();
        if !type_names
            .clone()
            .chain(predefined_type_names.iter())
            .all_unique()
        {
            let type_name_counts = type_names.collect::<Counter<_>>();
            for (name, count) in type_name_counts {
                if predefined_type_names.contains(name) {
                    return Err(format!("Attempt to override built-in type name {}", name));
                }
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
                    let variant_names = items.iter().map(|item| &item.id);
                    if !variant_names.clone().all_unique() {
                        let variant_name_counts = variant_names.collect::<Counter<_>>();
                        for (name, count) in variant_name_counts {
                            if count > 1 {
                                return Err(format!("Duplicated variant name {}", name));
                            }
                        }
                        panic!("Variant names were not unique but all counts were < 2");
                    }
                    let variants = items.iter().map(|item| Variant {
                        id: item.id.clone(),
                        type_: item.type_.as_ref().map(|type_instance| {
                            TypeChecker::convert_ast_type(type_instance, &type_definitions)
                        }),
                    });
                    Type::Union(variants.collect())
                }
                Definition::TransparentTypeDefinition(TransparentTypeDefinition {
                    variable: _,
                    type_,
                }) => TypeChecker::convert_ast_type(type_, &type_definitions),
                Definition::EmptyTypeDefinition(EmptyTypeDefinition { id: _ }) => Type::Empty,
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
        EmptyTypeDefinition, FunctionType, TransparentTypeDefinition, TupleType, TypeItem,
        TypeVariable, Typename, UnionTypeDefinition, ATOMIC_TYPE_BOOL, ATOMIC_TYPE_INT,
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
                variable: TypeVariable("Int"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: TypeVariable("Bool"),
                type_: ATOMIC_TYPE_BOOL.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (Id::from("Int"), TYPE_INT),
            (Id::from("Bool"), TYPE_BOOL),
        ]));
        "two type definitions"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("int"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
        ],
        None;
        "additional int definition"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: TypeVariable("bool"),
                items: vec![
                    TypeItem { id: Id::from("two"), type_: None},
                    TypeItem { id: Id::from("four"), type_: None},
                ]
            }.into()
        ],
        None;
        "additional bool definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("ii"),
                type_: TupleType{
                    types: vec![ATOMIC_TYPE_INT.into(),ATOMIC_TYPE_INT.into()]
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("ii"),
                Type::Tuple(vec![TYPE_INT, TYPE_INT])
            ),
        ]));
        "tuple type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: TypeVariable("i2b"),
                type_: FunctionType{
                    argument_type: Box::new(ATOMIC_TYPE_INT.into()),
                    return_type: Box::new(ATOMIC_TYPE_BOOL.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("i2b"),
                Type::Function(Box::new(TYPE_INT), Box::new(TYPE_BOOL))
            ),
        ]));
        "function type definition"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition {
                variable: TypeVariable("u2u"),
                type_: FunctionType{
                    argument_type: Box::new(TupleType{types: Vec::new()}.into()),
                    return_type: Box::new(TupleType{types: Vec::new()}.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("u2u"),
                Type::Function(Box::new(Type::Tuple(Vec::new())), Box::new(Type::Tuple(Vec::new())))
            ),
        ]));
        "transparent function type definition"
    )]
    #[test_case(
        vec![
            EmptyTypeDefinition{id: Id::from("None")}.into()
        ],
        Some(
            TypeDefinitions::from([
                (
                    Id::from("None"),
                    Type::Empty
                )
            ])
        );
        "empty type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: TypeVariable("iint"),
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: TypeVariable("iiint"),
                type_: Typename("iint").into(),
            }.into(),
        ],
        Some(
            TypeDefinitions::from({
                let iint = Rc::new(RefCell::new(TYPE_INT));
                let iiint = Rc::new(RefCell::new(Type::Reference(iint.clone())));
                [(Id::from("iint"), iint), (Id::from("iiint"), iiint)]
            })
        );
        "indirect type reference"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: TypeVariable("left"),
                items: vec![
                    TypeItem{
                        id: Id::from("Right"),
                        type_: Some(
                            TupleType{
                                types: vec![
                                    Typename("right").into(),
                                    ATOMIC_TYPE_BOOL.into()
                                ]
                            }.into()
                        )
                    },
                    TypeItem{
                        id: Id::from("Incorrect"),
                        type_: None
                    }
                ]
            }.into(),
            UnionTypeDefinition{
                variable: TypeVariable("right"),
                items: vec![
                    TypeItem{
                        id: Id::from("Left"),
                        type_: Some(Typename("left").into())
                    },
                    TypeItem{
                        id: Id::from("Correct"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        Some(
            TypeDefinitions::from({
                let left = Rc::new(RefCell::new(Type::Empty));
                let right = Rc::new(RefCell::new(
                    Type::Union(vec![
                        Variant{
                            id: Id::from("Left"),
                            type_: Some(
                                Type::Reference(left.clone())
                            )
                        },
                        Variant{
                            id: Id::from("Correct"),
                            type_: None
                        }
                    ])
                ));
                *left.borrow_mut() = Type::Union(vec![
                    Variant{
                        id: Id::from("Right"),
                        type_: Some(
                            Type::Tuple(vec![
                                Type::Reference(right.clone()),
                                TYPE_BOOL
                            ])
                        )
                    },
                    Variant{
                        id: Id::from("Incorrect"),
                        type_: None
                    }
                ]);
                [(Id::from("left"), left), (Id::from("right"), right)]
            })
        );
        "mutually recursive types"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: TypeVariable("Left_Right"),
                items: vec![
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    }
                ]
            }.into(),
        ],
        None;
        "duplicate types in union type"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: TypeVariable("Left_Right"),
                items: vec![
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                    TypeItem{
                        id: Id::from("left"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        None;
        "duplicate names in union type"
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
