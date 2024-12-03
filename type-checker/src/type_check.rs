use crate::type_check_nodes::{
    ParametricExpression, ParametricType, Type, TypeCheckError, TypedAssignment, TypedBlock,
    TypedElementAccess, TypedExpression, TypedIf, TypedTuple, TypedVariable, TYPE_BOOL,
};
use crate::utils::UniqueError;
use crate::{
    utils, AtomicType, AtomicTypeEnum, Block, Definition, ElementAccess, EmptyTypeDefinition,
    Expression, FunctionType, GenericType, GenericTypeVariable, GenericVariable, Id, IfExpression,
    OpaqueTypeDefinition, TransparentTypeDefinition, TupleExpression, TupleType, TypeInstance,
    UnionTypeDefinition,
};
use counter::Counter;
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;
use strum::IntoEnumIterator;

type TypeReferencesIndex = HashMap<*mut ParametricType, Id>;

type K = Id;
type V = Rc<RefCell<ParametricType>>;

#[derive(Clone)]
struct TypeDefinitions(HashMap<K, V>);

impl TypeDefinitions {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn get(&self, k: &Id) -> Option<&V> {
        self.0.get(k)
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.0.get_mut(k)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.0.insert(k, v)
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
            (Type::Atomic(a1), Type::Atomic(a2)) => a1 == a2,
            (Type::Union(v1), Type::Union(v2)) => {
                v1.len() == v2.len()
                    && v1
                        .iter()
                        .sorted_by_key(|(i1, _)| *i1)
                        .zip(v2.iter().sorted_by_key(|(i1, _)| *i1))
                        .all(|((i1, o1), (i2, o2))| {
                            i1 == i2
                                && match (&o1, &o2) {
                                    (None, None) => true,
                                    (Some(t1), Some(t2)) => TypeDefinitions::type_equality(
                                        self_references_index,
                                        other_references_index,
                                        t1,
                                        t2,
                                    ),
                                    _ => false,
                                }
                        })
            }
            (Type::Instantiation(t1, i1), Type::Instantiation(t2, i2)) => {
                self_references_index.get(&t1.as_ptr()) == other_references_index.get(&t2.as_ptr())
                    && i1.len() == i2.len()
                    && i1.into_iter().zip(i2.into_iter()).all(|(t1, t2)| {
                        TypeDefinitions::type_equality(
                            self_references_index,
                            other_references_index,
                            t1,
                            t2,
                        )
                    })
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
            (Type::Variable(i1), Type::Variable(i2)) => i1 == i2,
            _ => false,
        }
    }
}

impl From<HashMap<Id, Rc<RefCell<ParametricType>>>> for TypeDefinitions {
    fn from(value: HashMap<Id, Rc<RefCell<ParametricType>>>) -> Self {
        TypeDefinitions(value)
    }
}

impl<const N: usize> From<[(Id, Rc<RefCell<ParametricType>>); N]> for TypeDefinitions {
    fn from(arr: [(Id, Rc<RefCell<ParametricType>>); N]) -> Self {
        HashMap::from(arr).into()
    }
}

impl FromIterator<(Id, ParametricType)> for TypeDefinitions {
    fn from_iter<T: IntoIterator<Item = (Id, ParametricType)>>(iter: T) -> Self {
        HashMap::from_iter(iter).into()
    }
}

impl From<HashMap<Id, ParametricType>> for TypeDefinitions {
    fn from(value: HashMap<Id, ParametricType>) -> Self {
        value
            .into_iter()
            .map(|(id, type_)| (id, Rc::from(RefCell::from(type_))))
            .collect::<HashMap<_, _>>()
            .into()
    }
}

impl<const N: usize> From<[(Id, ParametricType); N]> for TypeDefinitions {
    fn from(arr: [(Id, ParametricType); N]) -> Self {
        HashMap::from(arr).into()
    }
}

impl From<HashMap<Id, Type>> for TypeDefinitions {
    fn from(value: HashMap<Id, Type>) -> Self {
        value
            .into_iter()
            .map(|(id, type_)| (id, type_.into()))
            .collect::<HashMap<_, ParametricType>>()
            .into()
    }
}

impl<const N: usize> From<[(Id, Type); N]> for TypeDefinitions {
    fn from(arr: [(Id, Type); N]) -> Self {
        arr.into_iter().collect()
    }
}
impl FromIterator<(Id, Type)> for TypeDefinitions {
    fn from_iter<T: IntoIterator<Item = (Id, Type)>>(iter: T) -> Self {
        HashMap::from_iter(iter).into()
    }
}

impl fmt::Debug for TypeDefinitions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let references_index = Box::new(self.references_index());
        f.debug_map()
            .entries(self.0.iter().map(|(key, value)| {
                (
                    key,
                    DebugTypeWrapper(value.borrow().clone().type_, references_index.clone()),
                )
            }))
            .finish()
    }
}

struct DebugTypeWrapper(Type, Box<HashMap<*mut ParametricType, Id>>);
impl fmt::Debug for DebugTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let references_index = &self.1;
        match self.0.clone() {
            Type::Union(variants) => {
                write!(
                    f,
                    "Union({:?})",
                    variants
                        .into_iter()
                        .map(|(id, type_)| {
                            (
                                id,
                                type_
                                    .map(|type_| DebugTypeWrapper(type_, references_index.clone())),
                            )
                        })
                        .collect_vec()
                )
            }
            Type::Instantiation(rc, instances) => {
                write!(
                    f,
                    "Instantation({}, {:?})",
                    references_index
                        .get(&rc.as_ptr())
                        .unwrap_or(&Id::from("unknown")),
                    instances
                        .into_iter()
                        .map(|type_| DebugTypeWrapper(type_, references_index.clone()))
                        .collect_vec()
                )
            }
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
            type_ => write!(f, "{:?}", type_),
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
                (Some(t1), Some(t2)) => {
                    let p1 = &*&t1.borrow();
                    let p2 = &*&t2.borrow();
                    p1.num_parameters == p2.num_parameters
                        && TypeDefinitions::type_equality(
                            self_references_index,
                            other_references_index,
                            &p1.type_,
                            &p2.type_,
                        )
                }
                _ => false,
            })
    }
}

type TypeContext = TypeDefinitions;

struct TypeChecker {}

impl TypeChecker {
    fn convert_ast_type(
        type_instance: &TypeInstance,
        type_definitions: &TypeDefinitions,
        generic_variables: &Vec<Id>,
    ) -> Result<Type, TypeCheckError> {
        Ok(match type_instance {
            TypeInstance::AtomicType(AtomicType {
                type_: atomic_type_enum,
            }) => Type::Atomic(atomic_type_enum.clone()),
            TypeInstance::GenericType(GenericType { id, type_variables }) => {
                if let Some(position) = generic_variables.iter().position(|variable| variable == id)
                {
                    if type_variables == &Vec::new() {
                        Type::Variable(position as u32)
                    } else {
                        return Err(TypeCheckError::DefaultError(format!(
                            "Attempt to instantiate type variable {} with {:?}",
                            id, type_variables
                        )));
                    }
                } else if let Some(reference) = type_definitions.get(id) {
                    if type_variables.len() as u32 != reference.borrow().num_parameters {
                        let type_name = type_definitions
                            .references_index()
                            .get(&reference.as_ptr())
                            .cloned();
                        return Err(TypeCheckError::DefaultError(format!(
                            "{} accepts {} type parameters but called with {:?} ({})",
                            type_name.unwrap_or(String::from("unknown")),
                            reference.borrow().num_parameters,
                            type_variables,
                            type_variables.len()
                        )));
                    }
                    Type::Instantiation(
                        reference.clone(),
                        type_variables
                            .into_iter()
                            .map(|type_instance| {
                                TypeChecker::convert_ast_type(
                                    type_instance,
                                    type_definitions,
                                    generic_variables,
                                )
                            })
                            .collect::<Result<_, _>>()?,
                    )
                } else {
                    return Err(TypeCheckError::DefaultError(format!(
                        "{} is not a valid type or generic name",
                        id
                    )));
                }
            }
            TypeInstance::TupleType(TupleType { types }) => Type::Tuple(
                types
                    .iter()
                    .map(|t| TypeChecker::convert_ast_type(t, type_definitions, generic_variables))
                    .collect::<Result<_, _>>()?,
            ),
            TypeInstance::FunctionType(FunctionType {
                argument_type,
                return_type,
            }) => Type::Function(
                Box::new(TypeChecker::convert_ast_type(
                    &argument_type,
                    type_definitions,
                    generic_variables,
                )?),
                Box::new(TypeChecker::convert_ast_type(
                    &return_type,
                    type_definitions,
                    generic_variables,
                )?),
            ),
        })
    }
    fn check_type_definitions(
        definitions: &Vec<Definition>,
    ) -> Result<TypeDefinitions, TypeCheckError> {
        let type_names = definitions.iter().map(Definition::get_id);
        let all_type_parameters = definitions.iter().map(Definition::get_parameters);
        let predefined_type_names = AtomicTypeEnum::iter()
            .map(|a| AtomicTypeEnum::to_string(&a).to_lowercase())
            .collect_vec();
        if !type_names
            .clone()
            .chain(predefined_type_names.iter())
            .all_unique()
        {
            let type_name_counts = type_names.collect::<Counter<_>>();
            for (nam, count) in type_name_counts {
                if predefined_type_names.contains(nam) {
                    return Err(TypeCheckError::DefaultError(format!(
                        "Attempt to override built-in type name {}",
                        nam
                    )));
                }
                if count > 1 {
                    return Err(TypeCheckError::DefaultError(format!(
                        "Duplicated type name {}",
                        nam
                    )));
                }
            }
            panic!("Type names were not unique but all counts were < 2");
        }
        for (type_name, type_parameters) in type_names.clone().zip(all_type_parameters.clone()) {
            if type_parameters.contains(type_name) {
                return Err(TypeCheckError::DefaultError(format!(
                    "Type {} contains itself as a parameter",
                    type_name
                )));
            }
        }
        for type_parameters in all_type_parameters.clone() {
            if !type_parameters
                .iter()
                .chain(predefined_type_names.iter())
                .all_unique()
            {
                let type_parameter_counts = type_parameters.into_iter().collect::<Counter<_>>();
                for (parameter, count) in type_parameter_counts {
                    if predefined_type_names.contains(&parameter) {
                        return Err(TypeCheckError::DefaultError(format!(
                            "Attempt to override built-in type name {}",
                            parameter
                        )));
                    }
                    if count > 1 {
                        return Err(TypeCheckError::DefaultError(format!(
                            "Duplicated parameter name {}",
                            parameter
                        )));
                    }
                }
                panic!("Type names were not unique but all counts were < 2");
            }
        }
        let mut type_definitions: TypeDefinitions = type_names
            .zip(all_type_parameters)
            .map(|(id, parameters)| {
                (
                    id.clone(),
                    ParametricType {
                        type_: Type::new(),
                        num_parameters: parameters.len() as u32,
                    },
                )
            })
            .collect();
        for definition in definitions {
            let type_name = definition.get_id();
            let type_ = match &definition {
                Definition::OpaqueTypeDefinition(OpaqueTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables,
                        },
                    type_,
                }) => Type::Union(HashMap::from([(
                    id.clone(),
                    Some(TypeChecker::convert_ast_type(
                        type_,
                        &type_definitions,
                        generic_variables,
                    )?),
                )])),
                Definition::UnionTypeDefinition(UnionTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id: _,
                            generic_variables,
                        },
                    items,
                }) => {
                    let variant_names = items.iter().map(|item| &item.id);
                    match utils::check_unique(variant_names.clone()) {
                        Ok(()) => {}
                        Err(UniqueError { duplicate }) => {
                            return Err(TypeCheckError::DuplicatedNameError {
                                duplicate: duplicate.clone(),
                                type_: String::from("variant name"),
                            })
                        }
                    }
                    let variants = items.iter().map(|item| {
                        item.type_
                            .as_ref()
                            .map(|type_instance| {
                                TypeChecker::convert_ast_type(
                                    type_instance,
                                    &type_definitions,
                                    generic_variables,
                                )
                            })
                            .transpose()
                            .map(|type_| (item.id.clone(), type_))
                    });
                    Type::Union(variants.collect::<Result<_, _>>()?)
                }
                Definition::TransparentTypeDefinition(TransparentTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id: _,
                            generic_variables,
                        },
                    type_,
                }) => TypeChecker::convert_ast_type(type_, &type_definitions, generic_variables)?,
                Definition::EmptyTypeDefinition(EmptyTypeDefinition { id }) => {
                    Type::Union(HashMap::from([(id.clone(), None)]))
                }
            };
            if let Some(type_reference) = type_definitions.get_mut(type_name) {
                type_reference.borrow_mut().type_ = type_;
            } else {
                panic!("{} not found in type definitions", type_name)
            }
        }
        return Ok(type_definitions);
    }
    fn check_expression(
        expression: &Expression,
        context: &TypeContext,
    ) -> Result<TypedExpression, TypeCheckError> {
        Ok(match expression {
            Expression::Integer(i) => i.clone().into(),
            Expression::Boolean(b) => b.clone().into(),
            Expression::TupleExpression(TupleExpression { expressions }) => TypedTuple {
                expressions: expressions
                    .iter()
                    .map(|expression| TypeChecker::check_expression(expression, context))
                    .collect::<Result<_, _>>()?,
            }
            .into(),
            Expression::GenericVariable(GenericVariable { id, type_instances }) => {
                let variable_type = context.get(id);
                match variable_type {
                    Some(type_) => {
                        if type_instances.len() != type_.borrow().num_parameters as usize {
                            return Err(TypeCheckError::DefaultError(format!("wrong number parameters to instantiate type {} (expected {} got {})", id, type_.borrow().num_parameters, type_instances.len())))
                        }
                        let type_ = if type_instances.len() == 0 {
                            type_.borrow().type_.clone()
                        } else {
                            Type::Instantiation(
                                type_.clone(),
                                type_instances.iter()
                                .map(|type_instance| TypeChecker::convert_ast_type(type_instance, &TypeDefinitions::new(), &Vec::new()))
                                .collect::<Result<_,_>>()?)
                        };
                        TypedVariable {
                            id: id.clone(),
                            type_
                        }
                    }.into(),
                    None => return Err(TypeCheckError::DefaultError(format!("{} not found in scope", id))),
                }
            }
            Expression::ElementAccess(ElementAccess { expression, index }) => {
                let typed_expression = TypeChecker::check_expression(expression, context)?;
                match typed_expression.type_() {
                    Type::Tuple(types) => {
                        if *index as usize >= types.len() {
                            return Err(TypeCheckError::DefaultError(format!(
                                "Cannot access element {}, {:?} contains only {} elements",
                                index,
                                expression,
                                types.len()
                            )));
                        };
                        TypedElementAccess {
                            expression: Box::new(typed_expression),
                            index: *index,
                        }
                        .into()
                    }
                    _ => {
                        return Err(TypeCheckError::InvalidAccessError {
                            expression: typed_expression,
                        })
                    }
                }
            }
            Expression::IfExpression(IfExpression {
                condition,
                true_block,
                false_block,
            }) => {
                let condition = TypeChecker::check_expression(&*condition, context)?;
                if condition.type_() != TYPE_BOOL {
                    return Err(TypeCheckError::InvalidConditionError { condition });
                }
                let typed_true_block = TypeChecker::check_block(true_block, context)?;
                let typed_false_block = TypeChecker::check_block(false_block, context)?;
                if typed_true_block.type_() != typed_false_block.type_() {
                    return Err(TypeCheckError::NonMatchingIfBlocksError {
                        true_block: typed_true_block,
                        false_block: typed_false_block,
                    });
                }
                TypedIf {
                    condition: Box::new(condition),
                    true_block: typed_true_block,
                    false_block: typed_false_block,
                }
                .into()
            }
            _ => todo!(),
        })
    }
    fn check_block(block: &Block, context: &TypeContext) -> Result<TypedBlock, TypeCheckError> {
        let mut new_context = context.clone();
        let mut assignments = Vec::new();
        let assignment_names = block
            .assignments
            .iter()
            .map(|assignment| assignment.assignee.id.clone());
        match utils::check_unique(assignment_names) {
            Ok(()) => {}
            Err(UniqueError { duplicate }) => {
                return Err(TypeCheckError::DuplicatedNameError {
                    duplicate,
                    type_: String::from("assignment names"),
                })
            }
        }
        for assignment in &block.assignments {
            let typed_expression =
                TypeChecker::check_expression(&assignment.expression, &new_context)?;
            let assignment = TypedAssignment {
                id: assignment.assignee.id.clone(),
                expression: ParametricExpression {
                    expression: typed_expression,
                    num_parameters: assignment.assignee.generic_variables.len() as u32,
                },
            };
            new_context.insert(
                assignment.id.clone(),
                Rc::new(RefCell::new(ParametricType {
                    type_: (assignment.expression.expression.type_()),
                    num_parameters: assignment.expression.num_parameters,
                })),
            );
            assignments.push(assignment);
        }
        Ok(TypedBlock {
            assignments,
            expression: Box::new(TypeChecker::check_expression(
                &block.expression,
                &new_context,
            )?),
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        type_check_nodes::{TYPE_BOOL, TYPE_INT, TYPE_UNIT},
        Assignment, Block, Boolean, ElementAccess, ExpressionBlock, GenericTypeVariable,
        IfExpression, Integer, TupleExpression, TypeItem, TypeVariable, Typename, Variable,
        VariableAssignee, ATOMIC_TYPE_BOOL, ATOMIC_TYPE_INT,
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
            (Id::from("i"), Type::Union(HashMap::from([
                (Id::from("i"), Some(TYPE_INT))
            ])))
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
                    TypeItem {
                        id: Id::from("Int"),
                        type_: Some(ATOMIC_TYPE_INT.into())
                    },
                    TypeItem {
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
                    HashMap::from([
                        (Id::from("Int"), Some(TYPE_INT.into())),
                        (Id::from("Bool"), Some(TYPE_BOOL.into()))
                    ])
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
                    let reference = Rc::new(RefCell::new(ParametricType::new()));
                    let union_type = Type::Union(HashMap::from([
                        (
                            Id::from("Cons"),
                            Some(Type::Instantiation(Rc::clone(&reference), Vec::new())),
                        ),
                        (
                            Id::from("Nil"),
                            None,
                        ),
                    ]));
                    *reference.borrow_mut() = union_type.into();
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
            (
                Id::from("Int"),
                Type::Union(HashMap::from([
                    (Id::from("Int"), Some(TYPE_INT))
                ]))
            ),
            (
                Id::from("Bool"),
                Type::Union(HashMap::from([
                    (Id::from("Bool"), Some(TYPE_BOOL))
                ]))
            ),
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
                Type::Union(HashMap::from([
                    (Id::from("ii"), Some(Type::Tuple(vec![TYPE_INT, TYPE_INT])))
                ]))
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
                Type::Union(HashMap::from([
                    (Id::from("i2b"), Some(Type::Function(Box::new(TYPE_INT), Box::new(TYPE_BOOL))))
                ]))
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
                    Type::Union(HashMap::from([
                        (Id::from("None"), None)
                    ]))
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
                let iint = Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(Id::from("iint"), Some(TYPE_INT))])).into()
                ));
                let iiint = Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(
                        Id::from("iiint"),
                        Some(Type::Instantiation(iint.clone(), Vec::new()))
                    )])).into()
                ));
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
                let left = Rc::new(RefCell::new(ParametricType::new()));
                let right = Rc::new(RefCell::new(
                    Type::Union(HashMap::from([
                        (
                            Id::from("Left"),
                            Some(
                                Type::Instantiation(left.clone(), Vec::new())
                            )
                        ),
                        (
                            Id::from("Correct"),
                            None
                        )
                    ])).into()
                ));
                *left.borrow_mut() = Type::Union(HashMap::from([
                    (
                        Id::from("Right"),
                        Some(
                            Type::Tuple(vec![
                                Type::Instantiation(right.clone(), Vec::new()),
                                TYPE_BOOL
                            ])
                        )
                    ),
                    (
                        Id::from("Incorrect"),
                        None
                    )
                ])).into();
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
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("wrapper"),
                    ParametricType{
                        type_: Type::Union(HashMap::from([(
                            Id::from("wrapper"),
                            Some(Type::Variable(0))
                        )])),
                        num_parameters: 1
                    }
                )]
            )
        );
        "opaque generic type test"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("transparent"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("transparent"),
                    ParametricType{
                        type_: Type::Variable(0),
                        num_parameters: 1
                    }
                )]
            )
        );
        "transparent generic type test"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Either"),
                    generic_variables: vec![String::from("T"), String::from("U")]
                },
                items: vec![
                    TypeItem {
                        id: String::from("Left"),
                        type_: Some(
                            Typename("T").into()
                        )
                    },
                    TypeItem {
                        id: String::from("Right"),
                        type_: Some(
                            Typename("U").into()
                        )
                    }
                ]
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("Either"),
                    ParametricType{
                        type_: Type::Union(HashMap::from([
                            (
                                Id::from("Left"),
                                Some(Type::Variable(0))
                            ),
                            (
                                Id::from("Right"),
                                Some(Type::Variable(1))
                            ),
                        ])),
                        num_parameters: 2
                    }
                )]
            )
        );
        "union generic type test"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: TypeVariable("Zero"),
                type_: Typename("Unknown").into(),
            }.into()
        ],
        None;
        "unknown type name"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Zero"),
                    generic_variables: vec![String::from("U")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        None;
        "unknown type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("T"), String::from("U"), String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
        ],
        None;
        "duplicate type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("int")]
                },
                type_: Typename("T").into()
            }.into(),
        ],
        None;
        "invalid type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("One")]
                },
                type_: Typename("One").into()
            }.into(),
        ],
        None;
        "type parameter same as name"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("U"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("V"),
                    generic_variables: vec![String::from("U")]
                },
                type_: Typename("U").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [
                    (
                        Id::from("U"),
                        ParametricType{
                            type_: Type::Union(HashMap::from([(
                                Id::from("U"),
                                Some(Type::Variable(0))
                            )])),
                            num_parameters: 1
                        }
                    ),
                    (
                        Id::from("V"),
                        ParametricType{
                            type_: Type::Union(HashMap::from([(
                                Id::from("V"),
                                Some(Type::Variable(0))
                            )])),
                            num_parameters: 1
                        }
                    ),
                ]
            )
        );
        "type parameter name override"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: TypeVariable("generic_int"),
                type_: GenericType{
                    id: Id::from("wrapper"),
                    type_variables: vec![ATOMIC_TYPE_INT.into()]
                }.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from({
                let wrapper = Rc::new(RefCell::new(ParametricType{
                    type_: Type::Union(HashMap::from([(
                        Id::from("wrapper"),
                        Some(Type::Variable(0))
                    )])),
                    num_parameters: 1
                }));
                let generic_int = Rc::new(RefCell::new(ParametricType{
                    type_: Type::Instantiation(wrapper.clone(), vec![TYPE_INT]),
                    num_parameters: 0
                }));
                [(Id::from("wrapper"), wrapper), (Id::from("generic_int"), generic_int)]
            })
        );
        "generic type instantiation"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: TypeVariable("generic_int"),
                type_: GenericType{
                    id: Id::from("wrapper"),
                    type_variables: vec![]
                }.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        None;
        "generic type instantiation wrong arguments"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("apply"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: GenericType{
                    id: Id::from("T"),
                    type_variables: vec![Typename("U").into()]
                }.into()
            }.into(),
        ],
        None;
        "generic type parameter instantiation"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Pair"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: TupleType{
                    types: vec![Typename("T").into(), Typename("U").into()]
                }.into()
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Pair"),
                ParametricType{
                    num_parameters: 2,
                    type_: Type::Tuple(
                        vec![Type::Variable(0), Type::Variable(1)]
                    )
                },
            )])
        );
        "pair type"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Function"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: FunctionType{
                    argument_type: Box::new(Typename("T").into()),
                    return_type: Box::new(Typename("U").into())
                }.into()
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Function"),
                ParametricType{
                    num_parameters: 2,
                    type_: Type::Function(
                        Box::new(Type::Variable(0)),
                        Box::new(Type::Variable(1))
                    )
                },
            )])
        );
        "function type"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Tree"),
                    generic_variables: vec![Id::from("T")]
                },
                items: vec![
                    TypeItem {
                        id: Id::from("Node"),
                        type_: Some(TupleType {
                            types: vec![
                                Typename("T").into(),
                                GenericType{
                                    id: Id::from("Tree"),
                                    type_variables: vec![Typename("T").into()]
                                }.into(),
                                Typename("T").into()
                            ]
                        }.into())
                    },
                    TypeItem {
                        id: Id::from("Leaf"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Tree"),
                {
                    let tree_type = Rc::new(RefCell::new(ParametricType{num_parameters: 1, type_: Type::new()}));
                    tree_type.borrow_mut().type_ = Type::Union(HashMap::from([
                        (
                            Id::from("Node"),
                            Some(Type::Tuple(vec![
                                Type::Variable(0),
                                Type::Instantiation(
                                    tree_type.clone(),
                                    vec![Type::Variable(0)]
                                ),
                                Type::Variable(0),
                            ]))
                        ),
                        (Id::from("Leaf"), None)
                    ]));
                    tree_type
                }
            )])
        );
        "tree type"
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
                if type_check_result.is_ok() {
                    println!("{:?}", type_check_result)
                }
                assert!(type_check_result.is_err())
            }
        }
    }

    const ALPHA_TYPE: Lazy<Rc<RefCell<ParametricType>>> = Lazy::new(|| {
        Rc::new(RefCell::new(ParametricType {
            type_: Type::Variable(0),
            num_parameters: 1,
        }))
    });
    #[test_case(
        Integer{value: -5}.into(),
        Some(TYPE_INT),
        TypeContext::new();
        "type check integer"
    )]
    #[test_case(
        Boolean{value: true}.into(),
        Some(TYPE_BOOL),
        TypeContext::new();
        "type check boolean"
    )]
    #[test_case(
        TupleExpression{
            expressions: Vec::new()
        }.into(),
        Some(Type::Tuple(Vec::new())),
        TypeContext::new();
        "type check empty tuple"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                Boolean{
                    value: true,
                }.into(),
                Integer{
                    value: -2,
                }.into(),
            ]
        }.into(),
        Some(Type::Tuple(vec![
            TYPE_BOOL.into(),
            TYPE_INT.into(),
        ])),
        TypeContext::new();
        "type check flat tuple"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                TupleExpression{
                    expressions: Vec::new()
                }.into(),
                TupleExpression{
                    expressions: vec![
                        Boolean{
                            value: true,
                        }.into(),
                        Integer{
                            value: -2,
                        }.into(),
                    ]
                }.into()
            ]
        }.into(),
        Some(Type::Tuple(vec![
            Type::Tuple(Vec::new()),
            Type::Tuple(vec![
                TYPE_BOOL.into(),
                TYPE_INT.into(),
            ])
        ])),
        TypeContext::new();
        "type check nested tuple"
    )]
    #[test_case(
        Variable("a").into(),
        Some(TYPE_INT),
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT)
            )
        ]);
        "type check variable"
    )]
    #[test_case(
        Variable("b").into(),
        None,
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT)
            )
        ]);
        "type check missing variable"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                Variable("b").into(),
                Variable("a").into(),
                Variable("a").into(),
            ]
        }.into(),
        Some(Type::Tuple(vec![
            TYPE_BOOL.into(),
            TYPE_INT.into(),
            TYPE_INT.into(),
        ])),
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT)
            ),
            (
                Id::from("b"),
                Type::from(TYPE_BOOL)
            )
        ]);
        "type check multiple variables"
    )]
    #[test_case(
        Variable("f").into(),
        None,
        TypeContext::from([
            (
                Id::from("f"),
                ParametricType{
                    type_: Type::Variable(0),
                    num_parameters: 1
                }
            )
        ]);
        "type check wrong arguments"
    )]
    #[test_case(
        GenericVariable {
            id: Id::from("f"),
            type_instances: vec![ATOMIC_TYPE_INT.into()]
        }.into(),
        Some(Type::Instantiation(ALPHA_TYPE.clone(), vec![TYPE_INT.into()])),
        TypeContext::from([
            (
                Id::from("f"),
                ALPHA_TYPE.clone()
            )
        ]);
        "type check parametric type"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(Integer{value:5}.into()),
            index: 0
        }.into(),
        None,
        TypeContext::new();
        "invalid type element access"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(TupleExpression{
                expressions: vec![
                    Integer{value: 5}.into(),
                    Boolean{value: true}.into(),
                ]
            }.into()),
            index: 0
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::new();
        "flat element access"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(TupleExpression{
                expressions: vec![
                    Integer{value: 5}.into(),
                    Boolean{value: true}.into(),
                ]
            }.into()),
            index: 2
        }.into(),
        None,
        TypeContext::new();
        "element access out of range"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(ElementAccess {
                expression: Box::new(Variable("a").into()),
                index: 0
            }.into()),
            index: 0
        }.into(),
        Some(TYPE_UNIT.into()),
        TypeContext::from([(
            Id::from("a"),
            Type::Tuple(
                vec![Type::Tuple(
                    vec![
                        TYPE_UNIT
                    ]
                )]
            )
        )]);
        "nested element access"
    )]
    #[test_case(
        Variable("empty").into(),
        Some(Type::Union([(
            Id::from("Empty"),
            None
        )].into())),
        TypeContext::from([(
            Id::from("empty"),
            Type::Union([(
                Id::from("Empty"),
                None
            )].into())
        )]);
        "variable with empty type"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Integer{value: 0}.into()),
            true_block: ExpressionBlock(Boolean{value: true}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression invalid condition"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Boolean{value: true}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        Some(TYPE_BOOL.into()),
        TypeContext::new();
        "if expression no assignments condition"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Integer{value: 8}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression different blocks"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Variable("x").into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression invalid block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: Box::new(VariableAssignee("x")),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Variable("x").into())
            },
            false_block: ExpressionBlock(Integer{value: 5}.into())
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::new();
        "if expression variable in block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: Box::new(VariableAssignee("x")),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Variable("x").into())
            },
            false_block: ExpressionBlock(Integer{value: 5}.into())
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::from([(
            Id::from("x"),
            TYPE_BOOL
        )]);
        "if expression variable shadowed in block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: Box::new(VariableAssignee("x")),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Variable("x").into())
            },
            false_block: ExpressionBlock(Variable("x").into())
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            TYPE_BOOL
        )]);
        "if expression variable shadowed incorrectly block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: Box::new(VariableAssignee("x")),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Variable("x").into())
            },
            false_block: ExpressionBlock(
                ElementAccess {
                    expression: Box::new(Variable("x").into()),
                    index: 1
                }.into()
            ),
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("x"),
            Type::Tuple(
                vec![
                    TYPE_BOOL,
                    TYPE_INT,
                ]
            )
        )]);
        "if expression variable shadowed then accessed"
    )]
    fn test_check_expressions(
        expression: Expression,
        expected_type: Option<Type>,
        context: TypeContext,
    ) {
        let type_check_result = TypeChecker::check_expression(&expression, &context);
        match expected_type {
            Some(type_) => match &type_check_result {
                Ok(typed_expression) => {
                    assert_eq!(typed_expression.type_(), type_)
                }
                Err(msg) => {
                    dbg!(msg);
                    assert!(&type_check_result.is_ok());
                }
            },
            None => {
                if type_check_result.is_ok() {
                    dbg!(&type_check_result);
                }
                assert!(&type_check_result.is_err());
            }
        }
    }

    #[test_case(
        ExpressionBlock(Boolean{value: true}.into()),
        Some(TYPE_BOOL),
        TypeContext::new();
        "block no assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Boolean{value: true}.into())
                }
            ],
            expression: Box::new(Boolean{value: true}.into())
        },
        Some(TYPE_BOOL),
        TypeContext::new();
        "block unused assignment"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Boolean{value: true}.into())
                }
            ],
            expression: Box::new(Variable("x").into())
        },
        Some(TYPE_BOOL),
        TypeContext::new();
        "block used assignment"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: Box::new(VariableAssignee("y")),
                    expression: Box::new(Variable("x").into())
                },
            ],
            expression: Box::new(Variable("y").into())
        },
        Some(TYPE_INT),
        TypeContext::new();
        "block multiple assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Integer{value: 5}.into())
                },
            ],
            expression: Box::new(Integer{value: 7}.into())
        },
        None,
        TypeContext::new();
        "block duplicate assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: Box::new(VariableAssignee("y")),
                    expression: Box::new(Variable("x").into())
                },
                Assignment{
                    assignee: Box::new(VariableAssignee("x")),
                    expression: Box::new(Integer{value: 3}.into())
                },
            ],
            expression: Box::new(Variable("y").into())
        },
        None,
        TypeContext::new();
        "block flipped assignments"
    )]
    fn test_check_blocks(block: Block, expected_type: Option<Type>, context: TypeContext) {
        let type_check_result = TypeChecker::check_block(&block, &context);
        match expected_type {
            Some(type_) => match &type_check_result {
                Ok(typed_expression) => {
                    assert_eq!(typed_expression.type_(), type_)
                }
                Err(msg) => {
                    dbg!(msg);
                    assert!(&type_check_result.is_ok());
                }
            },
            None => {
                if type_check_result.is_ok() {
                    dbg!(&type_check_result);
                }
                assert!(&type_check_result.is_err());
            }
        }
    }
}
