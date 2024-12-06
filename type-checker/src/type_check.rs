use crate::type_check_nodes::{
    ConstructorType, ParametricExpression, ParametricType, PartiallyTypedFunctionDefinition, Type,
    TypeCheckError, TypedAssignment, TypedBlock, TypedConstructorCall, TypedElementAccess,
    TypedExpression, TypedFunctionCall, TypedFunctionDefinition, TypedIf, TypedTuple,
    TypedVariable, TYPE_BOOL,
};
use crate::utils::UniqueError;
use crate::{
    utils, AtomicType, AtomicTypeEnum, Block, ConstructorCall, Definition, ElementAccess,
    EmptyTypeDefinition, Expression, FunctionCall, FunctionDefinition, FunctionType, GenericType,
    GenericTypeVariable, GenericVariable, Id, IfExpression, OpaqueTypeDefinition,
    TransparentTypeDefinition, TupleExpression, TupleType, TypeInstance, UnionTypeDefinition,
};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::hash_map::{IntoIter, Keys, Values};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::ops::Index;
use std::rc::Rc;
use strum::IntoEnumIterator;

type TypeReferencesIndex = HashMap<*mut ParametricType, Id>;
type GenericReferenceIndex = HashMap<*mut Option<Type>, usize>;

type K = Id;
type V = Rc<RefCell<ParametricType>>;
type V_ = Rc<RefCell<Option<Type>>>;

#[derive(Clone, Debug)]
struct GenericVariables(HashMap<Id, V_>);

impl GenericVariables {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn get(&self, k: &Id) -> Option<&V_> {
        self.0.get(k)
    }
    pub fn keys(&self) -> Keys<'_, K, V_> {
        self.0.keys()
    }
    pub fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (K, V_)>,
    {
        self.0.extend(iter)
    }
    pub fn into_iter(self) -> IntoIter<K, V_> {
        self.0.into_iter()
    }
}

impl Index<&K> for GenericVariables {
    type Output = V_;
    fn index<'a>(&'a self, index: &K) -> &'a V_ {
        &self.0[index]
    }
}

impl From<Vec<(K, V_)>> for GenericVariables {
    fn from(value: Vec<(K, V_)>) -> Self {
        value.into_iter().collect::<HashMap<_, _>>().into()
    }
}

impl From<HashMap<K, V_>> for GenericVariables {
    fn from(value: HashMap<K, V_>) -> Self {
        GenericVariables(value)
    }
}

impl From<&Vec<Id>> for GenericVariables {
    fn from(value: &Vec<Id>) -> Self {
        value
            .iter()
            .map(|variable| (variable.clone(), Rc::new(RefCell::new(None))))
            .collect::<HashMap<_, _>>()
            .into()
    }
}

impl From<(&Vec<Id>, &V)> for GenericVariables {
    fn from(value: (&Vec<Id>, &V)) -> Self {
        let (generic_variables, rc) = value;
        GenericVariables::from(
            generic_variables
                .iter()
                .zip(&rc.borrow().parameters)
                .map(|(id, rc)| (id.clone(), rc.clone()))
                .collect::<HashMap<_, _>>(),
        )
    }
}

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
    pub fn keys(&self) -> Keys<'_, K, V> {
        self.0.keys()
    }
    pub fn values(&self) -> Values<'_, K, V> {
        self.0.values()
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
        self_generics_index: &GenericReferenceIndex,
        other_generics_index: &GenericReferenceIndex,
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
                                        self_generics_index,
                                        other_generics_index,
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
                            self_generics_index,
                            other_generics_index,
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
                            self_generics_index,
                            other_generics_index,
                            t1,
                            t2,
                        )
                    })
            }
            (Type::Function(a1, r1), Type::Function(a2, r2)) => {
                TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    self_generics_index,
                    other_generics_index,
                    &Type::Tuple(a1.clone()),
                    &Type::Tuple(a2.clone()),
                ) && TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    self_generics_index,
                    other_generics_index,
                    r1,
                    r2,
                )
            }
            (Type::Variable(r1), Type::Variable(r2)) => {
                self_generics_index[&r1.as_ptr()] == other_generics_index[&r2.as_ptr()]
            }
            _ => false,
        }
    }
}

impl Index<&K> for TypeDefinitions {
    type Output = V;
    fn index<'a>(&'a self, index: &K) -> &'a V {
        &self.0[index]
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
                    (
                        value.borrow().clone().parameters,
                        DebugTypeWrapper(value.borrow().clone().type_, references_index.clone()),
                    ),
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
            Type::Function(argument_types, return_type) => {
                write!(
                    f,
                    "Function({:?},{:?})",
                    argument_types
                        .into_iter()
                        .map(|type_| DebugTypeWrapper(type_, references_index.clone()))
                        .collect_vec(),
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
                    p1.parameters.len() == p2.parameters.len()
                        && TypeDefinitions::type_equality(
                            self_references_index,
                            other_references_index,
                            &p1.parameters
                                .iter()
                                .enumerate()
                                .map(|(i, r)| (r.as_ptr(), i))
                                .collect(),
                            &p2.parameters
                                .iter()
                                .enumerate()
                                .map(|(i, r)| (r.as_ptr(), i))
                                .collect(),
                            &p1.type_,
                            &p2.type_,
                        )
                }
                _ => false,
            })
    }
}

type TypeContext = TypeDefinitions;

struct TypeChecker {
    type_definitions: TypeDefinitions,
    constructors: HashMap<Id, ConstructorType>,
}

impl TypeChecker {
    fn with_type_definitions(definitions: TypeDefinitions) -> Result<Self, TypeCheckError> {
        Ok(TypeChecker {
            constructors: TypeChecker::generate_constructors(&definitions)?,
            type_definitions: definitions,
        })
    }
    fn generate_constructors(
        definitions: &TypeDefinitions,
    ) -> Result<HashMap<Id, ConstructorType>, TypeCheckError> {
        let constructors = definitions
            .values()
            .map(|output_type| {
                if let Type::Union(items) = &output_type.borrow().type_ {
                    items
                        .into_iter()
                        .map(|(id, input_type)| {
                            (
                                id.clone(),
                                ConstructorType {
                                    input_type: input_type.clone(),
                                    output_type: output_type.borrow().type_.clone(),
                                    parameters: output_type.borrow().parameters.clone(),
                                },
                            )
                        })
                        .collect_vec()
                } else {
                    Vec::new()
                }
            })
            .concat();
        if let Err(utils::UniqueError { duplicate }) =
            utils::check_unique(constructors.iter().map(|(id, _)| id))
        {
            Err(TypeCheckError::DuplicatedName {
                duplicate: duplicate.clone(),
                reason: String::from("constructor"),
            })
        } else {
            Ok(constructors.into_iter().collect::<HashMap<_, _>>())
        }
    }
    fn convert_ast_type(
        type_instance: &TypeInstance,
        type_definitions: &TypeDefinitions,
        generic_variables: &GenericVariables,
    ) -> Result<Type, TypeCheckError> {
        Ok(match type_instance {
            TypeInstance::AtomicType(AtomicType {
                type_: atomic_type_enum,
            }) => Type::Atomic(atomic_type_enum.clone()),
            TypeInstance::GenericType(GenericType { id, type_variables }) => {
                if let Some(reference) = generic_variables.get(&id) {
                    if type_variables.is_empty() {
                        Type::Variable(reference.clone())
                    } else {
                        return Err(TypeCheckError::InstantiationOfTypeVariable {
                            variable: id.clone(),
                            type_instances: type_variables.clone(),
                        });
                    }
                } else if let Some(reference) = type_definitions.get(id) {
                    if type_variables.len() != reference.borrow().parameters.len() {
                        return Err(TypeCheckError::WrongNumberOfTypeParameters {
                            type_: reference.borrow().clone(),
                            type_instances: type_variables.clone(),
                        });
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
                    return Err(TypeCheckError::UnknownError {
                        id: id.clone(),
                        options: type_definitions
                            .keys()
                            .chain(generic_variables.keys())
                            .map(|id| id.clone())
                            .collect_vec(),
                        place: String::from("type name"),
                    });
                }
            }
            TypeInstance::TupleType(TupleType { types }) => Type::Tuple(
                types
                    .iter()
                    .map(|t| TypeChecker::convert_ast_type(t, type_definitions, generic_variables))
                    .collect::<Result<_, _>>()?,
            ),
            TypeInstance::FunctionType(FunctionType {
                argument_types,
                return_type,
            }) => Type::Function(
                argument_types
                    .iter()
                    .map(|type_| {
                        TypeChecker::convert_ast_type(type_, type_definitions, generic_variables)
                    })
                    .collect::<Result<_, _>>()?,
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
        if let Err(UniqueError { duplicate }) =
            utils::check_unique(type_names.clone().chain(predefined_type_names.iter()))
        {
            if predefined_type_names.contains(&duplicate) {
                return Err(TypeCheckError::BuiltInOverride {
                    name: duplicate.clone(),
                    reason: String::from("type name"),
                });
            } else {
                return Err(TypeCheckError::DuplicatedName {
                    duplicate: duplicate.clone(),
                    reason: String::from("type name"),
                });
            }
        }
        for (type_name, type_parameters) in type_names.clone().zip(all_type_parameters.clone()) {
            if type_parameters.contains(type_name) {
                return Err(TypeCheckError::TypeAsParameter {
                    type_name: type_name.clone(),
                });
            }
        }
        for type_parameters in all_type_parameters.clone() {
            if let Err(UniqueError { duplicate }) =
                utils::check_unique(type_parameters.iter().chain(predefined_type_names.iter()))
            {
                if predefined_type_names.contains(&duplicate) {
                    return Err(TypeCheckError::BuiltInOverride {
                        name: duplicate.clone(),
                        reason: String::from("type parameter"),
                    });
                } else {
                    return Err(TypeCheckError::DuplicatedName {
                        duplicate: duplicate.clone(),
                        reason: String::from("type parameter"),
                    });
                }
            }
        }
        let mut type_definitions: TypeDefinitions = type_names
            .zip(all_type_parameters)
            .map(|(id, parameters)| {
                (
                    id.clone(),
                    ParametricType {
                        type_: Type::new(),
                        parameters: (0..parameters.len())
                            .map(|_| Rc::new(RefCell::new(None)))
                            .collect_vec(),
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
                        &GenericVariables::from((generic_variables, &type_definitions[id])),
                    )?),
                )])),
                Definition::UnionTypeDefinition(UnionTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables,
                        },
                    items,
                }) => {
                    let variant_names = items.iter().map(|item| &item.id);
                    match utils::check_unique(variant_names.clone()) {
                        Ok(()) => {}
                        Err(UniqueError { duplicate }) => {
                            return Err(TypeCheckError::DuplicatedName {
                                duplicate: duplicate.clone(),
                                reason: String::from("variant name"),
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
                                    &GenericVariables::from((
                                        generic_variables,
                                        &type_definitions[id],
                                    )),
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
                            id,
                            generic_variables,
                        },
                    type_,
                }) => TypeChecker::convert_ast_type(
                    type_,
                    &type_definitions,
                    &GenericVariables::from((generic_variables, &type_definitions[id])),
                )?,
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
        for definition in definitions {
            if let Definition::TransparentTypeDefinition(TransparentTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                type_: _,
            }) = definition
            {
                if TypeChecker::is_self_recursive(id, &type_definitions).is_err() {
                    return Err(TypeCheckError::RecursiveTypeAlias {
                        type_alias: id.clone(),
                    });
                }
            }
        }
        return Ok(type_definitions);
    }
    fn is_self_recursive(id: &Id, definitions: &TypeDefinitions) -> Result<(), ()> {
        let start = definitions.get(id).unwrap();
        let mut queue = VecDeque::from([start.clone()]);
        let mut visited: HashMap<*mut ParametricType, bool> =
            HashMap::from_iter(definitions.values().map(|p| (p.as_ptr(), false)));
        fn update_queue(
            type_: &Type,
            start: &V,
            queue: &mut VecDeque<V>,
            visited: &mut HashMap<*mut ParametricType, bool>,
        ) -> Result<(), ()> {
            match type_ {
                Type::Union(items) => {
                    for type_ in items.values() {
                        if let Some(type_) = type_ {
                            update_queue(type_, start, queue, visited)?;
                        }
                    }
                }
                Type::Instantiation(rc, ts) => {
                    if rc.as_ptr() == start.as_ptr() {
                        return Err(());
                    }
                    if !visited.get(&rc.as_ptr()).unwrap() {
                        visited.insert(rc.as_ptr(), true);
                        queue.push_back(rc.clone());
                    }
                    update_queue(&Type::Tuple(ts.clone()), start, queue, visited)?
                }
                Type::Tuple(types) => {
                    for type_ in types {
                        update_queue(type_, start, queue, visited)?;
                    }
                }
                Type::Function(argument_types, return_type) => {
                    update_queue(&Type::Tuple(argument_types.clone()), start, queue, visited)?;
                    update_queue(&*return_type, start, queue, visited)?;
                }
                _ => (),
            }
            Ok(())
        }
        while let Some(rc) = queue.pop_front() {
            update_queue(&rc.borrow().type_, start, &mut queue, &mut visited)?;
        }
        Ok(())
    }
    fn check_expression(
        &self,
        expression: &Expression,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedExpression, TypeCheckError> {
        Ok(match expression {
            Expression::Integer(i) => i.clone().into(),
            Expression::Boolean(b) => b.clone().into(),
            Expression::TupleExpression(TupleExpression { expressions }) => TypedTuple {
                expressions: expressions
                    .iter()
                    .map(|expression| self.check_expression(expression, context, generic_variables))
                    .collect::<Result<_, _>>()?,
            }
            .into(),
            Expression::GenericVariable(GenericVariable { id, type_instances }) => {
                let variable_type = context.get(id);
                match variable_type {
                    Some(type_) => {
                        if type_instances.len() != type_.borrow().parameters.len() {
                            return Err(TypeCheckError::WrongNumberOfTypeParameters {
                                type_: type_.borrow().clone(),
                                type_instances: type_instances.clone(),
                            });
                        }
                        let type_ = if type_instances.is_empty() {
                            type_.borrow().type_.clone()
                        } else {
                            Type::Instantiation(
                                type_.clone(),
                                type_instances
                                    .iter()
                                    .map(|type_instance| {
                                        TypeChecker::convert_ast_type(
                                            type_instance,
                                            &self.type_definitions,
                                            generic_variables,
                                        )
                                    })
                                    .collect::<Result<_, _>>()?,
                            )
                        };
                        TypedVariable {
                            id: id.clone(),
                            type_,
                        }
                    }
                    .into(),
                    None => {
                        return Err(TypeCheckError::UnknownError {
                            place: String::from("variable"),
                            id: id.clone(),
                            options: context.keys().map(|id| id.clone()).collect_vec(),
                        })
                    }
                }
            }
            Expression::ElementAccess(ElementAccess { expression, index }) => {
                let typed_expression =
                    self.check_expression(expression, context, generic_variables)?;
                let Type::Tuple(types) = typed_expression.type_() else {
                    return Err(TypeCheckError::InvalidAccess {
                        expression: typed_expression,
                        index: *index,
                    });
                };
                if *index as usize >= types.len() {
                    return Err(TypeCheckError::InvalidAccess {
                        index: *index,
                        expression: typed_expression,
                    });
                };
                TypedElementAccess {
                    expression: Box::new(typed_expression),
                    index: *index,
                }
                .into()
            }
            Expression::IfExpression(IfExpression {
                condition,
                true_block,
                false_block,
            }) => {
                let condition = self.check_expression(&*condition, context, generic_variables)?;
                if condition.type_() != TYPE_BOOL {
                    return Err(TypeCheckError::InvalidCondition { condition });
                }
                let typed_true_block = self.check_block(true_block, context, generic_variables)?;
                let typed_false_block =
                    self.check_block(false_block, context, generic_variables)?;
                if typed_true_block.type_() != typed_false_block.type_() {
                    return Err(TypeCheckError::NonMatchingIfBlocks {
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
            Expression::FunctionDefinition(FunctionDefinition {
                parameters,
                return_type,
                body,
            }) => {
                let parameter_ids = parameters
                    .iter()
                    .map(|typed_assignee| typed_assignee.assignee.id.clone())
                    .collect_vec();
                if let Err(UniqueError { duplicate }) =
                    utils::check_unique::<_, &String>(parameter_ids.iter())
                {
                    return Err(TypeCheckError::DuplicatedName {
                        duplicate: duplicate.clone(),
                        reason: String::from("function parameter"),
                    });
                }
                let parameter_types = parameters
                    .iter()
                    .map(|typed_assignee| {
                        TypeChecker::convert_ast_type(
                            &typed_assignee.type_,
                            &self.type_definitions,
                            generic_variables,
                        )
                    })
                    .collect::<Result<_, _>>()?;
                PartiallyTypedFunctionDefinition {
                    parameter_types,
                    parameter_ids,
                    return_type: Box::new(TypeChecker::convert_ast_type(
                        return_type,
                        &self.type_definitions,
                        generic_variables,
                    )?),
                    body: body.clone(),
                }
                .into()
            }
            Expression::FunctionCall(FunctionCall {
                function,
                arguments,
            }) => {
                let function = self.check_expression(&function, context, generic_variables)?;
                let arguments_tuple = self.check_expression(
                    &TupleExpression {
                        expressions: arguments.clone(),
                    }
                    .into(),
                    context,
                    generic_variables,
                )?;
                let Type::Tuple(types) = arguments_tuple.type_() else {
                    panic!("Tuple expression has non-tuple type")
                };
                let TypedExpression::TypedTuple(TypedTuple {
                    expressions: arguments,
                }) = arguments_tuple
                else {
                    panic!("Tuple expression became non-tuple type.")
                };
                let Type::Function(argument_types, _) = function.type_() else {
                    return Err(TypeCheckError::InvalidFunctionCall {
                        expression: function,
                        arguments,
                    });
                };
                if argument_types != types {
                    return Err(TypeCheckError::InvalidFunctionCall {
                        expression: function,
                        arguments,
                    });
                }
                TypedFunctionCall {
                    function: Box::new(function),
                    arguments,
                }
                .into()
            }
            Expression::ConstructorCall(ConstructorCall {
                constructor,
                arguments,
            }) => {
                let Some(constructor_type) = self.constructors.get(&constructor.id) else {
                    return Err(TypeCheckError::UnknownError {
                        id: constructor.id.clone(),
                        options: self.constructors.keys().map(|id| id.clone()).collect_vec(),
                        place: String::from("type constructor"),
                    });
                };
                if constructor.type_instances.len() != constructor_type.parameters.len() {
                    return Err(TypeCheckError::WrongNumberOfTypeParameters {
                        type_: ParametricType {
                            type_: constructor_type.output_type.clone(),
                            parameters: constructor_type.parameters.clone(),
                        },
                        type_instances: constructor.type_instances.clone(),
                    });
                }
                let arguments_tuple = self.check_expression(
                    &TupleExpression {
                        expressions: arguments.clone(),
                    }
                    .into(),
                    context,
                    generic_variables,
                )?;
                let Type::Tuple(types) = arguments_tuple.type_() else {
                    panic!("Tuple expression has non-tuple type")
                };
                let TypedExpression::TypedTuple(TypedTuple {
                    expressions: arguments,
                }) = arguments_tuple
                else {
                    panic!("Tuple expression became non-tuple type.")
                };
                let Type::Tuple(type_variables) = TypeChecker::convert_ast_type(
                    &TypeInstance::TupleType(TupleType {
                        types: constructor.type_instances.clone(),
                    }),
                    &self.type_definitions,
                    generic_variables,
                )?
                else {
                    panic!("Tuple type converted to non-tuple type.");
                };
                let (input_type, output_type) = constructor_type.instantiate(&type_variables);
                match input_type {
                    Some(ref type_) => {
                        if vec![type_.clone()] != types {
                            return Err(TypeCheckError::InvalidConstructorArguments {
                                id: constructor.id.clone(),
                                input_type,
                                arguments,
                            });
                        }
                    }
                    None => {
                        if !types.is_empty() {
                            return Err(TypeCheckError::InvalidConstructorArguments {
                                id: constructor.id.clone(),
                                input_type,
                                arguments,
                            });
                        }
                    }
                }
                TypedConstructorCall {
                    id: constructor.id.clone(),
                    arguments,
                    output_type,
                }
                .into()
            }
            _ => todo!(),
        })
    }
    fn check_block(
        &self,
        block: &Block,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedBlock, TypeCheckError> {
        let mut new_context = context.clone();
        let mut assignments = Vec::new();
        let assignment_names = block
            .assignments
            .iter()
            .map(|assignment| assignment.assignee.id.clone());
        match utils::check_unique(assignment_names) {
            Ok(()) => {}
            Err(UniqueError { duplicate }) => {
                return Err(TypeCheckError::DuplicatedName {
                    duplicate,
                    reason: String::from("assignment name"),
                })
            }
        }
        for assignment in &block.assignments {
            let mut generic_variables = generic_variables.clone();
            generic_variables
                .extend(GenericVariables::from(&assignment.assignee.generic_variables).into_iter());
            let typed_expression =
                self.check_expression(&assignment.expression, &new_context, &generic_variables)?;
            let assignment = TypedAssignment {
                id: assignment.assignee.id.clone(),
                expression: ParametricExpression {
                    expression: typed_expression,
                    parameters: assignment
                        .assignee
                        .generic_variables
                        .iter()
                        .map(|id| (id.clone(), generic_variables[id].clone()))
                        .collect(),
                },
            };
            new_context.insert(
                assignment.id.clone(),
                Rc::new(RefCell::new(ParametricType {
                    type_: (assignment.expression.expression.type_()),
                    parameters: assignment
                        .expression
                        .parameters
                        .iter()
                        .map(|(_, rc)| rc.clone())
                        .collect_vec(),
                })),
            );
            assignments.push(assignment);
        }
        let typed_expression =
            self.check_expression(&block.expression, &new_context, generic_variables)?;
        let block = TypedBlock {
            assignments,
            expression: Box::new(typed_expression),
        };
        self.check_functions_in_block(&block, &new_context, generic_variables)
    }
    fn check_functions_in_expression(
        &self,
        expression: &TypedExpression,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedExpression, TypeCheckError> {
        Ok(match expression {
            TypedExpression::Integer(_)
            | TypedExpression::Boolean(_)
            | TypedExpression::TypedVariable(_) => expression.clone(),
            TypedExpression::TypedTuple(TypedTuple { expressions }) => {
                TypedExpression::TypedTuple(TypedTuple {
                    expressions: expressions
                        .iter()
                        .map(|expression| {
                            self.check_functions_in_expression(
                                expression,
                                context,
                                generic_variables,
                            )
                        })
                        .collect::<Result<_, _>>()?,
                })
            }
            TypedExpression::TypedElementAccess(TypedElementAccess { expression, index }) => {
                TypedExpression::TypedElementAccess(TypedElementAccess {
                    expression: Box::new(self.check_functions_in_expression(
                        &*expression,
                        context,
                        generic_variables,
                    )?),
                    index: *index,
                })
            }
            TypedExpression::TypedIf(TypedIf {
                condition,
                true_block,
                false_block,
            }) => TypedExpression::TypedIf(TypedIf {
                condition: Box::new(self.check_functions_in_expression(
                    condition,
                    context,
                    generic_variables,
                )?),
                true_block: self.check_functions_in_block(
                    true_block,
                    context,
                    generic_variables,
                )?,
                false_block: self.check_functions_in_block(
                    false_block,
                    context,
                    generic_variables,
                )?,
            }),
            TypedExpression::PartiallyTypedFunctionDefinition(
                partially_typed_function_definition,
            ) => TypedExpression::TypedFunctionDefinition(self.fully_type_function(
                partially_typed_function_definition,
                context,
                generic_variables,
            )?),
            TypedExpression::TypedFunctionDefinition(_) => {
                panic!("Typed function found when only partially typed functions are expected.")
            }
            TypedExpression::TypedFunctionCall(TypedFunctionCall {
                function,
                arguments,
            }) => {
                let TypedExpression::TypedTuple(TypedTuple {
                    expressions: arguments,
                }) = self.check_functions_in_expression(
                    &TypedTuple {
                        expressions: arguments.clone(),
                    }
                    .into(),
                    context,
                    generic_variables,
                )?
                else {
                    panic!("Tuple expression has non-tuple type.")
                };
                TypedFunctionCall {
                    function: Box::new(self.check_functions_in_expression(
                        &*function,
                        context,
                        generic_variables,
                    )?),
                    arguments,
                }
                .into()
            }
            TypedExpression::TypedConstructorCall(TypedConstructorCall {
                id,
                output_type,
                arguments,
            }) => {
                let TypedExpression::TypedTuple(TypedTuple {
                    expressions: arguments,
                }) = self.check_functions_in_expression(
                    &TypedTuple {
                        expressions: arguments.clone(),
                    }
                    .into(),
                    context,
                    generic_variables,
                )?
                else {
                    panic!("Tuple expression has non-tuple type.")
                };
                TypedConstructorCall {
                    id: id.clone(),
                    output_type: output_type.clone(),
                    arguments,
                }
                .into()
            }
        })
    }
    fn check_functions_in_block(
        &self,
        block: &TypedBlock,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedBlock, TypeCheckError> {
        Ok(TypedBlock {
            assignments: block
                .assignments
                .iter()
                .map(|assignment| {
                    let mut generic_variables = generic_variables.clone();
                    generic_variables.extend(
                        GenericVariables::from(assignment.expression.parameters.clone())
                            .into_iter(),
                    );
                    self.check_functions_in_expression(
                        &assignment.expression.expression,
                        context,
                        &generic_variables,
                    )
                    .map(|expression| TypedAssignment {
                        id: assignment.id.clone(),
                        expression: ParametricExpression {
                            expression,
                            parameters: assignment.expression.parameters.clone(),
                        },
                    })
                })
                .collect::<Result<_, _>>()?,
            expression: Box::new(self.check_functions_in_expression(
                &block.expression,
                context,
                generic_variables,
            )?),
        })
    }
    fn fully_type_function(
        &self,
        function_definition: &PartiallyTypedFunctionDefinition,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedFunctionDefinition, TypeCheckError> {
        let mut new_context = context.clone();
        for (parameter_name, parameter_type) in function_definition
            .parameter_ids
            .iter()
            .zip(function_definition.parameter_types.iter())
        {
            new_context.insert(
                parameter_name.clone(),
                Rc::new(RefCell::new(parameter_type.clone().into())),
            );
        }
        let body = self.check_block(&function_definition.body, &new_context, generic_variables)?;
        if *function_definition.return_type != body.type_() {
            return Err(TypeCheckError::FunctionReturnTypeMismatch {
                return_type: *function_definition.return_type.clone(),
                body,
            });
        }
        Ok(TypedFunctionDefinition {
            parameter_ids: function_definition.parameter_ids.clone(),
            parameter_types: function_definition.parameter_types.clone(),
            return_type: function_definition.return_type.clone(),
            body,
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        type_check_nodes::{ConstructorType, TYPE_BOOL, TYPE_INT, TYPE_UNIT},
        Assignee, Assignment, Block, Boolean, Constructor, ConstructorCall, ElementAccess,
        ExpressionBlock, FunctionCall, FunctionDefinition, GenericConstructor, GenericTypeVariable,
        IfExpression, Integer, TupleExpression, TypeItem, TypeVariable, TypedAssignee, Typename,
        Variable, VariableAssignee, ATOMIC_TYPE_BOOL, ATOMIC_TYPE_INT,
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
                    argument_types: vec![ATOMIC_TYPE_INT.into()],
                    return_type: Box::new(ATOMIC_TYPE_BOOL.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("i2b"),
                Type::Union(HashMap::from([
                    (Id::from("i2b"), Some(Type::Function(vec![TYPE_INT], Box::new(TYPE_BOOL))))
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
                    argument_types: Vec::new(),
                    return_type: Box::new(TupleType{types: Vec::new()}.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("u2u"),
                Type::Function(Vec::new(), Box::new(Type::Tuple(Vec::new())))
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
                    {
                        let parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::Union(HashMap::from([(
                                Id::from("wrapper"),
                                Some(Type::Variable(parameter.clone()))
                            )])),
                            parameters: vec![parameter]
                        }
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
                    {
                        let parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::Variable(parameter.clone()),
                            parameters: vec![parameter]
                        }
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
                    {
                        let left_parameter = Rc::new(RefCell::new(None));
                        let right_parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::Union(HashMap::from([
                                (
                                    Id::from("Left"),
                                    Some(Type::Variable(left_parameter.clone()))
                                ),
                                (
                                    Id::from("Right"),
                                    Some(Type::Variable(right_parameter.clone()))
                                ),
                            ])),
                            parameters: vec![left_parameter, right_parameter]
                        }
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
                        {
                            let parameter = Rc::new(RefCell::new(None));
                            ParametricType{
                                type_: Type::Union(HashMap::from([(
                                    Id::from("U"),
                                    Some(Type::Variable(parameter.clone()))
                                )])),
                                parameters: vec![parameter]
                            }
                        }
                    ),
                    (
                        Id::from("V"),
                        {
                            let parameter = Rc::new(RefCell::new(None));
                            ParametricType{
                                type_: Type::Union(HashMap::from([(
                                    Id::from("V"),
                                    Some(Type::Variable(parameter.clone()))
                                )])),
                                parameters: vec![parameter]
                            }
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
                let parameter = Rc::new(RefCell::new(None));
                let wrapper = Rc::new(RefCell::new(ParametricType{
                    type_: Type::Union(HashMap::from([(
                        Id::from("wrapper"),
                        Some(Type::Variable(parameter.clone()))
                    )])),
                    parameters: vec![parameter]
                }));
                let generic_int = Rc::new(RefCell::new(ParametricType{
                    type_: Type::Instantiation(wrapper.clone(), vec![TYPE_INT]),
                    parameters: Vec::new()
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
                {
                    let left_parameter = Rc::new(RefCell::new(None));
                    let right_parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        parameters: vec![left_parameter.clone(), right_parameter.clone()],
                        type_: Type::Tuple(
                            vec![Type::Variable(left_parameter), Type::Variable(right_parameter)]
                        )
                    }
                }
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
                    argument_types: vec![Typename("T").into()],
                    return_type: Box::new(Typename("U").into())
                }.into()
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Function"),
                {
                    let argument_parameter = Rc::new(RefCell::new(None));
                    let return_parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        parameters: vec![argument_parameter.clone(), return_parameter.clone()],
                        type_: Type::Function(
                            vec![Type::Variable(argument_parameter)],
                            Box::new(Type::Variable(return_parameter))
                        )
                    }
                }
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
                    let parameter = Rc::new(RefCell::new(None));
                    let tree_type = Rc::new(RefCell::new(ParametricType{parameters: vec![parameter.clone()], type_: Type::new()}));
                    tree_type.borrow_mut().type_ = Type::Union(HashMap::from([
                        (
                            Id::from("Node"),
                            Some(Type::Tuple(vec![
                                Type::Variable(parameter.clone()),
                                Type::Instantiation(
                                    tree_type.clone(),
                                    vec![Type::Variable(parameter.clone())]
                                ),
                                Type::Variable(parameter.clone()),
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
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: TypeVariable("recursive"),
                type_: Typename("recursive").into(),
            }.into(),
        ],
        None;
        "recursive typealias"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Recursive"),
                    generic_variables: vec![Id::from("T")]
                },
                type_: GenericType{
                    id: Id::from("Recursive"),
                    type_variables: vec![Typename("T").into()]
                }.into(),
            }.into(),
            TransparentTypeDefinition{
                variable: TypeVariable("recursive_alias"),
                type_: GenericType{
                    id: Id::from("Recursive"),
                    type_variables: vec![Typename("recursive_alias").into()]
                }.into(),
            }.into(),
        ],
        None;
        "indirectly recursive typealias"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: TypeVariable("recursive1"),
                type_: Typename("recursive2").into(),
            }.into(),
            TransparentTypeDefinition{
                variable: TypeVariable("recursive2"),
                type_: Typename("recursive1").into(),
            }.into(),
        ],
        None;
        "mutually recursive typealias"
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
            }.into(),
            TransparentTypeDefinition {
                variable: TypeVariable("int_list2"),
                type_: Typename("int_list").into()
            }.into()
        ],
        Some(TypeDefinitions::from(
            {
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
                let instantiation = Rc::new(RefCell::new(Type::Instantiation(reference.clone(), Vec::new()).into()));
                [
                    (Id::from("int_list"),reference),
                    (Id::from("int_list2"), instantiation)
                ]
            }
        ));
        "recursive type alias"
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
        let parameter = Rc::new(RefCell::new(None));
        Rc::new(RefCell::new(ParametricType {
            type_: Type::Variable(parameter.clone()),
            parameters: vec![parameter],
        }))
    });
    const TYPE_DEFINITIONS: Lazy<TypeDefinitions> = Lazy::new(|| {
        TypeDefinitions::from([
            (
                Id::from("opaque_int"),
                Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(Id::from("opaque_int"), Some(TYPE_INT))])).into(),
                )),
            ),
            (
                Id::from("transparent_int"),
                Rc::new(RefCell::new(TYPE_INT.into())),
            ),
            (
                Id::from("transparent_int_2"),
                Rc::new(RefCell::new(TYPE_INT.into())),
            ),
            (
                Id::from("ii"),
                Rc::new(RefCell::new(Type::Tuple(vec![TYPE_INT, TYPE_INT]).into())),
            ),
            (Id::from("recursive"), {
                let reference = Rc::new(RefCell::new(ParametricType::new()));
                *reference.borrow_mut() = Type::Union(HashMap::from([(
                    Id::from("recursive"),
                    Some(Type::Instantiation(Rc::clone(&reference), Vec::new())),
                )]))
                .into();
                reference
            }),
            (Id::from("List"), {
                let parameter = Rc::new(RefCell::new(None));
                let list_type = Rc::new(RefCell::new(ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::new(),
                }));
                list_type.borrow_mut().type_ = Type::Union(HashMap::from([
                    (
                        Id::from("Cons"),
                        Some(Type::Tuple(vec![
                            Type::Variable(parameter.clone()),
                            Type::Instantiation(
                                list_type.clone(),
                                vec![Type::Variable(parameter.clone())],
                            ),
                        ])),
                    ),
                    (Id::from("Nil"), None),
                ]));
                list_type
            }),
        ])
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
                {
                    let parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        type_: Type::Variable(parameter.clone()),
                        parameters: vec![parameter]
                    }
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
    #[test_case(
        FunctionDefinition {
            parameters: Vec::new(),
            return_type: TupleType{types: Vec::new()}.into(),
            body: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
        }.into(),
        Some(Type::Function(vec![], Box::new(TYPE_UNIT))),
        TypeContext::new();
        "unit function def"
    )]
    #[test_case(
        FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("y")),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Variable("x").into())
        }.into(),
        Some(Type::Function(vec![TYPE_INT, TYPE_BOOL], Box::new(TYPE_INT))),
        TypeContext::new();
        "arguments function def"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Variable("+").into()),
            arguments: vec![
                Integer{ value: 3}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("+"),
            Type::Function(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )
        )]);
        "addition function call"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Variable("+").into()),
            arguments: vec![
                Boolean{ value: true}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        None,
        TypeContext::from([(
            Id::from("+"),
            Type::Function(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )
        )]);
        "addition function call wrong type"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Variable("+").into()),
            arguments: vec![
                Integer{ value: 3}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("+"),
            Type::Function(
                vec![
                    Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int")).unwrap().clone(), Vec::new()),
                    Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int")).unwrap().clone(), Vec::new())
                ],
                Box::new(Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int")).unwrap().clone(), Vec::new()))
            )
        )]);
        "addition function call with aliases"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: vec![
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("opaque_int")).unwrap().clone(), Vec::new())),
        TypeContext::new();
        "constructor call"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: vec![
                Boolean{ value: true}.into(),
            ],
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong type arguments"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: Vec::new(),
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong number arguments"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("opaque_int"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![
                Integer{ value: 5}.into(),
            ],
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong type parameters"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Nil"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: Vec::new(),
        }.into(),
        Some(TYPE_DEFINITIONS[&Id::from("List")].borrow().instantiate(&vec![TYPE_INT])),
        TypeContext::new();
        "constructor call output generic"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Cons"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![
                TupleExpression {
                    expressions: vec![
                        Integer{value: 3}.into(),
                        ConstructorCall {
                            constructor: GenericConstructor{
                                id: Id::from("Nil"),
                                type_instances: vec![ATOMIC_TYPE_INT.into()]
                            },
                            arguments: Vec::new(),
                        }.into(),
                    ]
                }.into()
            ],
        }.into(),
        Some(TYPE_DEFINITIONS[&Id::from("List")].borrow().instantiate(&vec![TYPE_INT])),
        TypeContext::new();
        "constructor call generic"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Empty"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![],
        }.into(),
        None,
        TypeContext::new();
        "constructor call non-existant"
    )]
    fn test_check_expressions(
        expression: Expression,
        expected_type: Option<Type>,
        context: TypeContext,
    ) {
        let Ok(type_checker) = TypeChecker::with_type_definitions(TYPE_DEFINITIONS.clone()) else {
            panic!("Invalid type checker definition")
        };
        let type_check_result =
            type_checker.check_expression(&expression, &context, &GenericVariables::new());
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
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("y")),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Variable("z").into())
        }.into()),
        None,
        TypeContext::new();
        "function invalid block"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("y")),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Variable("y").into())
        }.into()),
        None,
        TypeContext::new();
        "function incorrect return type"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Integer{value: 5}.into())
        }.into()),
        None,
        TypeContext::new();
        "function duplicate parameter"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: Typename("opaque_int").into()
                },
            ],
            return_type: Typename("opaque_int").into(),
            body: ExpressionBlock(Variable("x").into()),
        }.into()),
        Some(Type::Function(
            vec![Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("opaque_int")).unwrap().clone(), Vec::new())],
            Box::new(Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("opaque_int")).unwrap().clone(), Vec::new()))
        )),
        TypeContext::new();
        "opaque type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: Typename("transparent_int").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Variable("x").into()),
        }.into()),
        Some(Type::Function(
            vec![Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int")).unwrap().clone(), Vec::new())],
            Box::new(TYPE_INT)
        )),
        TypeContext::new();
        "transparent type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: Typename("transparent_int").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Variable("x").into()),
        }.into()),
        Some(Type::Function(
            vec![Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int")).unwrap().clone(), Vec::new())],
            Box::new(Type::Instantiation(TYPE_DEFINITIONS.get(&Id::from("transparent_int_2")).unwrap().clone(), Vec::new()))
        )),
        TypeContext::new();
        "double transparent type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: Typename("ii").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(ElementAccess{
                expression: Box::new(Variable("x").into()),
                index: 0
            }.into()),
        }.into()),
        Some(Type::Function(
            vec![Type::Tuple(vec![TYPE_INT, TYPE_INT])],
            Box::new(TYPE_INT)
        )),
        TypeContext::new();
        "transparent type usage"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: Typename("recursive").into()
                },
            ],
            return_type: Typename("recursive").into(),
            body: ExpressionBlock(ElementAccess{
                expression: Box::new(Variable("x").into()),
                index: 0
            }.into()),
        }.into()),
        None,
        TypeContext::new();
        "invalid recursive type usage"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition{
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("y")),
                    type_: ATOMIC_TYPE_INT.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(FunctionCall {
                function: Box::new(Variable("+").into()),
                arguments: vec![
                    Variable("x").into(),
                    Variable("y").into(),
                ],
            }.into())
        }.into()),
        Some(Type::Function(
            vec![TYPE_INT, TYPE_INT],
            Box::new(TYPE_INT)
        )),
        TypeContext::from([(
            Id::from("+"),
            Type::Function(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )
        )]);
        "add function definition"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition{
            parameters: vec![
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("x")),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Box::new(VariableAssignee("y")),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(FunctionCall {
                function: Box::new(Variable("+").into()),
                arguments: vec![
                    Variable("x").into(),
                    Variable("y").into(),
                ],
            }.into())
        }.into()),
        None,
        TypeContext::from([(
            Id::from("+"),
            Type::Function(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )
        )]);
        "add invalid function definition"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("x"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(Integer {value: -12}.into())
                },
            ],
            expression: Box::new(
                GenericVariable {
                    id: Id::from("x"),
                    type_instances: vec![
                        ATOMIC_TYPE_INT.into()
                    ]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "generic instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("x"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(Integer {value: -12}.into())
                },
            ],
            expression: Box::new(
                GenericVariable {
                    id: Id::from("x"),
                    type_instances: vec![
                        ATOMIC_TYPE_INT.into(),
                        ATOMIC_TYPE_BOOL.into()
                    ]
                }.into()
            )
        },
        None,
        TypeContext::new();
        "extra variable generic instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("id"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Variable("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "generic function instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("id"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Variable("x").into())
                    }.into())
                },
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("id_"),
                        generic_variables: vec![Id::from("U")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("U").into(),
                            }
                        ],
                        return_type: Typename("U").into(),
                        body: ExpressionBlock(FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![Typename("U").into()]
                            }.into()),
                            arguments: vec![Variable("x").into()]
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id_"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "used generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("id"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: Block{
                            assignments: vec![
                                Assignment {
                                    assignee: Box::new(Assignee {
                                        id: Id::from("hold"),
                                        generic_variables: vec![Id::from("U")]
                                    }),
                                    expression: Box::new(FunctionDefinition{
                                        parameters: vec![
                                            TypedAssignee {
                                                assignee: Box::new(VariableAssignee("y")),
                                                type_: Typename("U").into(),
                                            }
                                        ],
                                        return_type: Typename("T").into(),
                                        body: ExpressionBlock(Variable("x").into())
                                    }.into())
                                },
                            ],
                            expression: Box::new(FunctionCall {
                                function: Box::new(GenericVariable{
                                    id: Id::from("hold"),
                                    type_instances: vec![ATOMIC_TYPE_BOOL.into()]
                                }.into()),
                                arguments: vec![
                                    Boolean{value: false}.into()
                                ]
                            }.into())
                        }
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "nested generic function instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("id"),
                        generic_variables: vec![Id::from("T")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Variable("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(Variable("&").into()),
                    arguments: vec![
                        FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![ATOMIC_TYPE_INT.into()]
                            }.into()),
                            arguments: vec![Integer{value: 5}.into()]
                        }.into(),
                        FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![ATOMIC_TYPE_BOOL.into()]
                            }.into()),
                            arguments: vec![Boolean{value: false}.into()]
                        }.into()
                    ]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("&"),
            Type::Function(vec![TYPE_INT, TYPE_BOOL], Box::new(TYPE_INT))
        )]);
        "reused generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("apply"),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("f")),
                                type_: FunctionType{
                                    argument_types: vec![Typename("T").into()],
                                    return_type: Box::new(Typename("U").into())
                                }.into(),
                            },
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("U").into(),
                        body: ExpressionBlock(FunctionCall {
                            function: Box::new(Variable("f").into()),
                            arguments: vec![Variable("x").into()]
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("apply"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::Function(
            vec![
                Type::Function(vec![TYPE_INT], Box::new(TYPE_BOOL)),
                TYPE_INT
            ],
            Box::new(TYPE_BOOL)
        )),
        TypeContext::new();
        "compound generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("extra"),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Variable("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("extra"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::Function(
            vec![
                TYPE_INT
            ],
            Box::new(TYPE_INT)
        )),
        TypeContext::new();
        "dual generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: Box::new(Assignee {
                        id: Id::from("first"),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    }),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Box::new(VariableAssignee("x")),
                                type_: TupleType{
                                    types: vec![Typename("T").into(), Typename("U").into()],
                                }.into()
                            },
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(ElementAccess {
                            expression: Box::new(Variable("x").into()),
                            index: 0
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("first"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::Function(
            vec![
                Type::Tuple(vec![TYPE_INT, TYPE_BOOL]),
            ],
            Box::new(TYPE_INT)
        )),
        TypeContext::new();
        "tuple generic function"
    )]
    fn test_check_blocks(block: Block, expected_type: Option<Type>, context: TypeContext) {
        let Ok(type_checker) = TypeChecker::with_type_definitions(TYPE_DEFINITIONS.clone()) else {
            panic!("Invalid type checker definition")
        };
        let type_check_result =
            type_checker.check_block(&block, &context, &GenericVariables::new());
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

    #[test]
    fn test_valid_constructor_list() {
        let mut type_definitions = TypeDefinitions::from([
            (Id::from("Tree"), {
                let parameter = Rc::new(RefCell::new(None));
                let tree_type = Rc::new(RefCell::new(ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::new(),
                }));
                tree_type.borrow_mut().type_ = Type::Union(HashMap::from([
                    (
                        Id::from("Node"),
                        Some(Type::Tuple(vec![
                            Type::Variable(parameter.clone()),
                            Type::Instantiation(
                                tree_type.clone(),
                                vec![Type::Variable(parameter.clone())],
                            ),
                            Type::Variable(parameter.clone()),
                        ])),
                    ),
                    (Id::from("Leaf"), None),
                ]));
                tree_type
            }),
            (
                Id::from("Empty"),
                Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(Id::from("Empty"), None)])).into(),
                )),
            ),
            (
                Id::from("opaque_int"),
                Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(Id::from("opaque_int"), Some(TYPE_INT))])).into(),
                )),
            ),
        ]);
        type_definitions.insert(
            Id::from("opaque_opaque_int"),
            Rc::new(RefCell::new(
                Type::Union(HashMap::from([(
                    Id::from("opaque_opaque_int"),
                    Some(Type::Instantiation(
                        type_definitions[&Id::from("opaque_int")].clone(),
                        Vec::new(),
                    )),
                )]))
                .into(),
            )),
        );
        type_definitions.insert(
            Id::from("int_tree"),
            Rc::new(RefCell::new(
                Type::Union(HashMap::from([(
                    Id::from("int_tree"),
                    Some(Type::Instantiation(
                        type_definitions[&Id::from("Tree")].clone(),
                        vec![TYPE_INT],
                    )),
                )]))
                .into(),
            )),
        );
        let expected_constructors = HashMap::from([
            {
                let tree_type = type_definitions[&Id::from("Tree")].clone();
                let tree_parameter = tree_type.borrow().parameters[0].clone();
                (
                    Id::from("Node"),
                    ConstructorType {
                        input_type: Some(Type::Tuple(vec![
                            Type::Variable(tree_parameter.clone()),
                            Type::Instantiation(
                                tree_type.clone(),
                                vec![Type::Variable(tree_parameter.clone())],
                            ),
                            Type::Variable(tree_parameter.clone()),
                        ])),
                        output_type: tree_type.clone().borrow().type_.clone(),
                        parameters: vec![tree_parameter],
                    },
                )
            },
            (Id::from("Leaf"), {
                let tree_type = type_definitions[&Id::from("Tree")].clone();
                let tree_parameter = tree_type.borrow().parameters[0].clone();
                ConstructorType {
                    input_type: None,
                    output_type: tree_type.clone().borrow().type_.clone(),
                    parameters: vec![tree_parameter],
                }
            }),
            (
                Id::from("Empty"),
                ConstructorType {
                    input_type: None,
                    output_type: type_definitions[&Id::from("Empty")].borrow().type_.clone(),
                    parameters: Vec::new(),
                },
            ),
            (
                Id::from("opaque_int"),
                ConstructorType {
                    input_type: Some(TYPE_INT),
                    output_type: type_definitions[&Id::from("opaque_int")]
                        .borrow()
                        .type_
                        .clone(),
                    parameters: Vec::new(),
                },
            ),
            (
                Id::from("opaque_opaque_int"),
                ConstructorType {
                    input_type: Some(Type::Instantiation(
                        type_definitions[&Id::from("opaque_int")].clone(),
                        Vec::new(),
                    )),
                    output_type: type_definitions[&Id::from("opaque_opaque_int")]
                        .borrow()
                        .type_
                        .clone(),
                    parameters: Vec::new(),
                },
            ),
            (
                Id::from("int_tree"),
                ConstructorType {
                    input_type: Some(Type::Instantiation(
                        type_definitions[&Id::from("Tree")].clone(),
                        vec![TYPE_INT],
                    )),
                    output_type: Type::Instantiation(
                        type_definitions[&Id::from("int_tree")].clone(),
                        Vec::new(),
                    ),
                    parameters: Vec::new(),
                },
            ),
        ]);
        let Ok(type_checker) = TypeChecker::with_type_definitions(type_definitions) else {
            panic!("Invalid type checker definition");
        };
        assert_eq!(type_checker.constructors, expected_constructors)
    }

    #[test]
    fn test_invalid_constructor_list() {
        let type_definitions = TypeDefinitions::from([
            (Id::from("Tree"), {
                let parameter = Rc::new(RefCell::new(None));
                let tree_type = Rc::new(RefCell::new(ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::new(),
                }));
                tree_type.borrow_mut().type_ = Type::Union(HashMap::from([
                    (
                        Id::from("Node"),
                        Some(Type::Tuple(vec![
                            Type::Variable(parameter.clone()),
                            Type::Instantiation(
                                tree_type.clone(),
                                vec![Type::Variable(parameter.clone())],
                            ),
                            Type::Variable(parameter.clone()),
                        ])),
                    ),
                    (Id::from("Leaf"), None),
                ]));
                tree_type
            }),
            (
                Id::from("Leaf"),
                Rc::new(RefCell::new(
                    Type::Union(HashMap::from([(Id::from("Leaf"), None)])).into(),
                )),
            ),
        ]);
        let result = TypeChecker::with_type_definitions(type_definitions);
        assert!(result.is_err())
    }
}
