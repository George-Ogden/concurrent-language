use crate::{Assignee, AtomicTypeEnum, Block, Boolean, Id, Integer, MatchBlock, TypeInstance};
use from_variants::FromVariants;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::hash_map::{IntoIter, Keys, Values};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug};
use std::ops::Index;
use std::rc::Rc;

#[derive(PartialEq, Clone, Debug)]
pub struct ParametricType {
    pub type_: Type,
    pub parameters: Vec<Rc<RefCell<Option<Type>>>>,
}

impl From<Type> for ParametricType {
    fn from(value: Type) -> Self {
        ParametricType {
            type_: value,
            parameters: Vec::new(),
        }
    }
}

impl ParametricType {
    pub fn new() -> Self {
        ParametricType {
            type_: Type::new(),
            parameters: Vec::new(),
        }
    }
    pub fn instantiate(&self, type_variables: &Vec<Type>) -> Type {
        for (parameter, variable) in self.parameters.iter().zip_eq(type_variables) {
            *parameter.borrow_mut() = Some(variable.clone());
        }
        let type_ = self.type_.instantiate();
        for parameter in &self.parameters {
            *parameter.borrow_mut() = None;
        }
        type_
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedParametricVariable {
    pub variable: Variable,
    pub type_: Rc<RefCell<ParametricType>>,
}

impl From<Rc<RefCell<ParametricType>>> for TypedParametricVariable {
    fn from(value: Rc<RefCell<ParametricType>>) -> Self {
        TypedParametricVariable {
            variable: Rc::new(RefCell::new(())),
            type_: value,
        }
    }
}

impl From<ParametricType> for TypedParametricVariable {
    fn from(value: ParametricType) -> Self {
        TypedParametricVariable::from(Rc::new(RefCell::new(value)))
    }
}

impl From<Type> for TypedParametricVariable {
    fn from(value: Type) -> Self {
        TypedParametricVariable::from(ParametricType::from(value))
    }
}

impl From<TypedVariable> for TypedParametricVariable {
    fn from(value: TypedVariable) -> Self {
        TypedParametricVariable {
            variable: value.variable,
            type_: Rc::new(RefCell::new(value.type_.into())),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedVariable {
    pub variable: Variable,
    pub type_: Type,
}

impl From<Type> for TypedVariable {
    fn from(value: Type) -> Self {
        TypedVariable {
            variable: Rc::new(RefCell::new(())),
            type_: value,
        }
    }
}

#[derive(Clone)]
pub enum Type {
    Atomic(AtomicTypeEnum),
    Union(Id, Vec<Option<Type>>),
    Instantiation(Rc<RefCell<ParametricType>>, Vec<Type>),
    Tuple(Vec<Type>),
    Function(Vec<Type>, Box<Type>),
    Variable(Rc<RefCell<Option<Type>>>),
}

impl PartialEq for Type {
    fn eq(&self, other: &Type) -> bool {
        return Type::type_equality(self, other, &mut HashMap::new());
    }
}

impl Type {
    pub fn new() -> Self {
        Type::Tuple(Vec::new())
    }
    pub fn instantiate_types(types: &Vec<Self>) -> Vec<Type> {
        types.iter().map(Type::instantiate).collect_vec()
    }
    pub fn instantiate(&self) -> Type {
        match self {
            Self::Atomic(_) => self.clone(),
            Self::Tuple(types) => Type::Tuple(Type::instantiate_types(types)),
            Self::Union(id, types) => Type::Union(
                id.clone(),
                types
                    .iter()
                    .map(|type_| type_.clone().map(|type_| type_.instantiate()))
                    .collect(),
            ),
            Self::Instantiation(parametric_type, types) => {
                Type::Instantiation(parametric_type.clone(), Self::instantiate_types(types))
            }
            Self::Function(arg_types, return_type) => Type::Function(
                Self::instantiate_types(arg_types),
                Box::new(return_type.instantiate()),
            ),
            Self::Variable(i) => i.borrow().clone().unwrap_or(self.clone()),
        }
    }
    pub fn types_equality(
        t1: &Vec<Self>,
        t2: &Vec<Self>,
        equal_references: &mut HashMap<*mut ParametricType, *mut ParametricType>,
    ) -> bool {
        t1.len() == t2.len()
            && t1
                .iter()
                .zip_eq(t2)
                .all(|(t1, t2)| Type::type_equality(t1, t2, equal_references))
    }
    pub fn option_type_equality(
        t1: &Option<Self>,
        t2: &Option<Self>,
        equal_references: &mut HashMap<*mut ParametricType, *mut ParametricType>,
    ) -> bool {
        match (t1, t2) {
            (None, None) => true,
            (Some(t1), Some(t2)) => Type::type_equality(t1, t2, equal_references),
            _ => false,
        }
    }
    pub fn type_equality(
        t1: &Self,
        t2: &Self,
        equal_references: &mut HashMap<*mut ParametricType, *mut ParametricType>,
    ) -> bool {
        match (t1, t2) {
            (Self::Instantiation(r1, t1), Self::Instantiation(r2, t2))
                if r1.as_ptr() == r2.as_ptr() =>
            {
                Type::types_equality(t1, t2, equal_references)
            }
            (Self::Instantiation(r1, t1), Self::Instantiation(r2, t2))
                if r1.as_ptr() != r2.as_ptr() =>
            {
                if equal_references.get(&r1.as_ptr()) == Some(&r2.as_ptr()) {
                    true
                } else {
                    equal_references.insert(r1.as_ptr(), r2.as_ptr());
                    Type::type_equality(
                        &r1.borrow().instantiate(t1),
                        &r2.borrow().instantiate(t2),
                        equal_references,
                    )
                }
            }
            (Self::Instantiation(r1, t1), t2) | (t2, Self::Instantiation(r1, t1)) => {
                Type::type_equality(t2, &r1.borrow().instantiate(t1), equal_references)
            }
            (Self::Atomic(a1), Self::Atomic(a2)) => a1 == a2,
            (Self::Union(i1, t1), Self::Union(i2, t2)) => {
                i1 == i2
                    && t1.len() == t2.len()
                    && t1
                        .iter()
                        .zip_eq(t2.iter())
                        .all(|(t1, t2)| Type::option_type_equality(t1, t2, equal_references))
            }
            (Self::Tuple(t1), Self::Tuple(t2)) => Type::types_equality(t1, t2, equal_references),
            (Self::Function(a1, r1), Self::Function(a2, r2)) => {
                Type::types_equality(a1, a2, equal_references)
                    && Type::type_equality(r1, r2, equal_references)
            }
            (Self::Variable(r1), Self::Variable(r2)) => {
                r1.as_ptr() == r2.as_ptr()
                    || Type::option_type_equality(&r1.borrow(), &r2.borrow(), equal_references)
            }
            _ => false,
        }
    }
}

pub const TYPE_INT: Type = Type::Atomic(AtomicTypeEnum::INT);
pub const TYPE_BOOL: Type = Type::Atomic(AtomicTypeEnum::BOOL);
pub const TYPE_UNIT: Type = Type::Tuple(Vec::new());

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Atomic(atomic_type) => write!(f, "Atomic({:?})", atomic_type),
            Type::Union(id, variants) => {
                write!(f, "Union({:?},{:?})", id, variants.iter().collect_vec())
            }
            Type::Instantiation(reference, instances) => {
                write!(
                    f,
                    "Instantiation({:p},{:?})",
                    Rc::as_ptr(reference),
                    instances
                )
            }
            Type::Tuple(types) => write!(f, "Tuple({:?})", types),
            Type::Function(argument_type, return_type) => {
                write!(f, "Function({:?},{:?})", argument_type, return_type)
            }
            Type::Variable(idx) => write!(f, "Variable({:?})", idx.as_ptr()),
        }
    }
}

type Variable = Rc<RefCell<()>>;

#[derive(Debug, PartialEq, Clone)]
pub struct TypedTuple {
    pub expressions: Vec<TypedExpression>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedAccess {
    pub variable: TypedVariable,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedElementAccess {
    pub expression: Box<TypedExpression>,
    pub index: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedIf {
    pub condition: Box<TypedExpression>,
    pub true_block: TypedBlock,
    pub false_block: TypedBlock,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatchItem {
    pub type_name: Id,
    pub assignee: Option<TypedVariable>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatchBlock {
    pub matches: Vec<TypedMatchItem>,
    pub block: TypedBlock,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatch {
    pub subject: Box<TypedExpression>,
    pub blocks: Vec<TypedMatchBlock>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PartiallyTypedFunctionDefinition {
    pub parameters: Vec<(Id, TypedVariable)>,
    pub return_type: Box<Type>,
    pub body: Block,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedFunctionDefinition {
    pub parameters: Vec<TypedVariable>,
    pub return_type: Box<Type>,
    pub body: TypedBlock,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedFunctionCall {
    pub function: Box<TypedExpression>,
    pub arguments: Vec<TypedExpression>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedConstructorCall {
    pub id: Id,
    pub output_type: Type,
    pub arguments: Vec<TypedExpression>,
}

#[derive(Debug, PartialEq, Clone, FromVariants)]
pub enum TypedExpression {
    Integer(Integer),
    Boolean(Boolean),
    TypedTuple(TypedTuple),
    TypedAccess(TypedAccess),
    TypedElementAccess(TypedElementAccess),
    TypedIf(TypedIf),
    TypedMatch(TypedMatch),
    PartiallyTypedFunctionDefinition(PartiallyTypedFunctionDefinition),
    TypedFunctionDefinition(TypedFunctionDefinition),
    TypedFunctionCall(TypedFunctionCall),
    TypedConstructorCall(TypedConstructorCall),
}

impl TypedExpression {
    pub fn type_(&self) -> Type {
        let type_ = match self {
            Self::Integer(_) => TYPE_INT,
            Self::Boolean(_) => TYPE_BOOL,
            Self::TypedTuple(TypedTuple { expressions }) => {
                Type::Tuple(expressions.iter().map(TypedExpression::type_).collect_vec())
            }
            Self::TypedAccess(TypedAccess {
                variable: TypedVariable { variable: _, type_ },
            }) => type_.clone(),
            Self::TypedElementAccess(TypedElementAccess { expression, index }) => {
                if let Type::Tuple(types) = expression.type_() {
                    types[*index as usize].clone()
                } else {
                    panic!("Type of an element access is no longer a tuple!")
                }
            }
            Self::TypedIf(TypedIf {
                condition: _,
                true_block,
                false_block: _,
            }) => true_block.type_(),
            Self::PartiallyTypedFunctionDefinition(PartiallyTypedFunctionDefinition {
                parameters,
                return_type,
                body: _,
            }) => Type::Function(
                parameters
                    .iter()
                    .map(|(_, parameter)| parameter.type_.clone())
                    .collect_vec(),
                return_type.clone(),
            ),
            Self::TypedFunctionDefinition(TypedFunctionDefinition {
                parameters,
                return_type,
                body: _,
            }) => Type::Function(
                parameters
                    .iter()
                    .map(|parameter| parameter.type_.clone())
                    .collect_vec(),
                return_type.clone(),
            ),
            Self::TypedFunctionCall(TypedFunctionCall {
                function,
                arguments: _,
            }) => {
                let Type::Function(_, return_type) = function.type_() else {
                    panic!("Function does not have function type.")
                };
                *return_type
            }
            Self::TypedConstructorCall(TypedConstructorCall {
                output_type,
                id: _,
                arguments: _,
            }) => output_type.clone(),
            Self::TypedMatch(TypedMatch { subject: _, blocks }) => {
                let Some(block) = blocks.first() else {
                    panic!("Match block with no blocks.")
                };
                block.block.type_()
            }
        };
        let type_ = if let Type::Instantiation(r, t) = type_ {
            r.borrow().instantiate(&t)
        } else {
            type_
        };
        type_
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ParametricExpression {
    pub expression: TypedExpression,
    pub parameters: Vec<(Id, Rc<RefCell<Option<Type>>>)>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedAssignment {
    pub variable: Variable,
    pub expression: ParametricExpression,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedBlock {
    pub assignments: Vec<TypedAssignment>,
    pub expression: Box<TypedExpression>,
}

impl TypedBlock {
    pub fn type_(&self) -> Type {
        return self.expression.type_();
    }
}

#[derive(Debug)]
pub struct TypedProgram {
    pub type_definitions: TypeDefinitions,
    pub main: TypedVariable,
    pub assignments: Vec<TypedAssignment>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeCheckError {
    DuplicatedName {
        duplicate: Id,
        reason: String,
    },
    InvalidCondition {
        condition: TypedExpression,
    },
    InvalidAccess {
        expression: TypedExpression,
        index: u32,
    },
    NonMatchingIfBlocks {
        true_block: TypedBlock,
        false_block: TypedBlock,
    },
    FunctionReturnTypeMismatch {
        return_type: Type,
        body: TypedBlock,
    },
    UnknownError {
        id: Id,
        options: Vec<Id>,
        place: String,
    },
    BuiltInOverride {
        name: Id,
        reason: String,
    },
    TypeAsParameter {
        type_name: Id,
    },
    RecursiveTypeAlias {
        type_alias: Id,
    },
    InvalidFunctionCall {
        expression: TypedExpression,
        arguments: Vec<TypedExpression>,
    },
    InstantiationOfTypeVariable {
        variable: Id,
        type_instances: Vec<TypeInstance>,
    },
    WrongNumberOfTypeParameters {
        type_: ParametricType,
        type_instances: Vec<TypeInstance>,
    },
    InvalidConstructorArguments {
        id: Id,
        input_type: Option<Type>,
        arguments: Vec<TypedExpression>,
    },
    DifferingMatchBlockTypes(TypedMatchBlock, TypedMatchBlock),
    NonUnionTypeMatchSubject(TypedExpression),
    IncorrectVariants {
        blocks: Vec<MatchBlock>,
    },
    MismatchedVariant {
        type_: Type,
        variant_id: Id,
        assignee: Option<Assignee>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorType {
    pub type_: Rc<RefCell<ParametricType>>,
    pub index: u32,
}

type TypeReferencesIndex = HashMap<*mut ParametricType, Id>;
type GenericReferenceIndex = HashMap<*mut Option<Type>, usize>;

type K = Id;
type V = Rc<RefCell<ParametricType>>;
type V_ = Rc<RefCell<Option<Type>>>;

#[derive(Clone, Debug)]
pub struct GenericVariables(HashMap<Id, V_>);

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
pub struct TypeDefinitions(HashMap<K, V>);

impl TypeDefinitions {
    pub fn get(&self, k: &Id) -> Option<&V> {
        self.0.get(k)
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.0.get_mut(k)
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
            (Type::Union(i1, v1), Type::Union(i2, v2)) => {
                i1 == i2
                    && v1.len() == v2.len()
                    && v1
                        .iter()
                        .zip_eq(v2.iter())
                        .all(|(i1, i2)| match (&i1, &i2) {
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
            Type::Union(id, variants) => {
                write!(
                    f,
                    "Union({:?},{:?})",
                    id,
                    variants
                        .into_iter()
                        .map(|type_| type_
                            .map(|type_| DebugTypeWrapper(type_, references_index.clone())))
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

pub type TypeContext = HashMap<Id, TypedParametricVariable>;
