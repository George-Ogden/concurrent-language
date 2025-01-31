use crate::{Assignee, AtomicTypeEnum, Boolean, Id, Integer, MatchBlock, TypeInstance};
use from_variants::FromVariants;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::hash_map::{IntoIter, Keys, Values};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::ops::Index;
use std::rc::Rc;

#[derive(PartialEq, Clone, Debug, Eq)]
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
            variable: Variable::new(),
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
    pub type_: ParametricType,
}

impl TypedVariable {
    fn instantiate(&self) -> Self {
        TypedVariable {
            variable: self.variable.clone(),
            type_: ParametricType {
                type_: self.type_.type_.instantiate(),
                parameters: self.type_.parameters.clone(),
            },
        }
    }
}

impl From<Rc<RefCell<ParametricType>>> for TypedVariable {
    fn from(value: Rc<RefCell<ParametricType>>) -> Self {
        TypedVariable {
            variable: Variable::new(),
            type_: value.borrow().clone(),
        }
    }
}

impl From<ParametricType> for TypedVariable {
    fn from(value: ParametricType) -> Self {
        TypedVariable {
            variable: Variable::new(),
            type_: value,
        }
    }
}

impl From<Type> for TypedVariable {
    fn from(value: Type) -> Self {
        ParametricType::from(value).into()
    }
}

macro_rules! strict_partial_eq {
    ($t:ty) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                Self::strict_equality(self, other, std::collections::HashSet::new())
            }
        }
    };
}

#[derive(Clone, Eq, Hash, FromVariants)]
pub enum Type {
    TypeAtomic(TypeAtomic),
    TypeUnion(TypeUnion),
    TypeInstantiation(TypeInstantiation),
    TypeTuple(TypeTuple),
    TypeFn(TypeFn),
    TypeVariable(TypeVariable),
}

impl From<AtomicTypeEnum> for Type {
    fn from(value: AtomicTypeEnum) -> Type {
        TypeAtomic(value).into()
    }
}

strict_partial_eq!(Type);

type Visited = HashSet<*mut ParametricType>;

impl Type {
    pub fn new() -> Self {
        TypeTuple(Vec::new()).into()
    }
    pub fn instantiate_types(types: &Vec<Self>) -> Vec<Type> {
        types.iter().map(Type::instantiate).collect_vec()
    }
    pub fn instantiate(&self) -> Type {
        match self {
            Self::TypeAtomic(_) => self.clone(),
            Self::TypeTuple(TypeTuple(types)) => TypeTuple(Type::instantiate_types(types)).into(),
            Self::TypeUnion(TypeUnion {
                id,
                variants: types,
            }) => TypeUnion {
                id: id.clone(),
                variants: types
                    .iter()
                    .map(|type_| type_.clone().map(|type_| type_.instantiate()))
                    .collect(),
            }
            .into(),
            Self::TypeInstantiation(TypeInstantiation {
                reference: parametric_type,
                instances: types,
            }) => TypeInstantiation {
                reference: parametric_type.clone(),
                instances: Self::instantiate_types(types),
            }
            .into(),
            Self::TypeFn(TypeFn(arg_types, return_type)) => TypeFn(
                Self::instantiate_types(arg_types),
                Box::new(return_type.instantiate()),
            )
            .into(),
            Self::TypeVariable(TypeVariable(v)) => v.borrow().clone().unwrap_or(self.clone()),
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
    pub fn equality(t1: &Self, t2: &Self) -> bool {
        Self::type_equality(t1, t2, &mut HashMap::new())
    }
    pub fn type_equality(
        t1: &Self,
        t2: &Self,
        equal_references: &mut HashMap<*mut ParametricType, *mut ParametricType>,
    ) -> bool {
        match (t1, t2) {
            (
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: t1,
                }),
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r2,
                    instances: t2,
                }),
            ) if r1.as_ptr() == r2.as_ptr() => Type::types_equality(t1, t2, equal_references),
            (
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: t1,
                }),
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r2,
                    instances: t2,
                }),
            ) if r1.as_ptr() != r2.as_ptr() => {
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
            (
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: t1,
                }),
                t2,
            )
            | (
                t2,
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: t1,
                }),
            ) => Type::type_equality(t2, &r1.borrow().instantiate(t1), equal_references),
            (Self::TypeAtomic(a1), Self::TypeAtomic(a2)) => a1 == a2,
            (
                Self::TypeUnion(TypeUnion {
                    id: i1,
                    variants: t1,
                }),
                Self::TypeUnion(TypeUnion {
                    id: i2,
                    variants: t2,
                }),
            ) => {
                i1 == i2
                    && t1.len() == t2.len()
                    && t1
                        .iter()
                        .zip_eq(t2.iter())
                        .all(|(t1, t2)| Type::option_type_equality(t1, t2, equal_references))
            }
            (Self::TypeTuple(TypeTuple(t1)), Self::TypeTuple(TypeTuple(t2))) => {
                Type::types_equality(t1, t2, equal_references)
            }
            (Self::TypeFn(TypeFn(a1, r1)), Self::TypeFn(TypeFn(a2, r2))) => {
                Type::types_equality(a1, a2, equal_references)
                    && Type::type_equality(r1, r2, equal_references)
            }
            (Self::TypeVariable(TypeVariable(r1)), Self::TypeVariable(TypeVariable(r2))) => {
                r1.as_ptr() == r2.as_ptr()
                    || Type::option_type_equality(&r1.borrow(), &r2.borrow(), equal_references)
            }
            _ => false,
        }
    }
    pub fn strict_equality(t1: &Self, t2: &Self, mut visited: Visited) -> bool {
        match (t1, t2) {
            (Type::TypeAtomic(a1), Type::TypeAtomic(a2)) => {
                TypeAtomic::strict_equality(a1, a2, visited)
            }
            (Type::TypeUnion(u1), Type::TypeUnion(u2)) => {
                TypeUnion::strict_equality(u1, u2, visited)
            }
            (Type::TypeTuple(t1), Type::TypeTuple(t2)) => {
                TypeTuple::strict_equality(t1, t2, visited)
            }
            (Type::TypeFn(f1), Type::TypeFn(f2)) => TypeFn::strict_equality(f1, f2, visited),
            (Type::TypeVariable(v1), Type::TypeVariable(v2)) => {
                TypeVariable::strict_equality(v1, v2, visited)
            }
            (
                Type::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: v1,
                }),
                Type::TypeInstantiation(TypeInstantiation {
                    reference: r2,
                    instances: v2,
                }),
            ) => {
                let p1 = r1.as_ptr();
                let p2 = r2.as_ptr();
                if p1 == p2 && Type::strict_equalities(v1, v2, visited.clone()) {
                    true
                } else if visited.contains(&p1) || visited.contains(&p2) {
                    false
                } else {
                    visited.insert(p1);
                    if Type::strict_equality(t2, &r1.borrow().instantiate(v1), visited.clone()) {
                        true
                    } else {
                        visited.insert(p2);
                        Type::strict_equality(t1, &r2.borrow().instantiate(v2), visited)
                    }
                }
            }
            (
                t2,
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: v1,
                }),
            )
            | (
                Self::TypeInstantiation(TypeInstantiation {
                    reference: r1,
                    instances: v1,
                }),
                t2,
            ) => {
                let p1 = r1.as_ptr();
                if visited.contains(&p1) {
                    false
                } else {
                    visited.insert(p1);
                    Type::strict_equality(t2, &r1.borrow().instantiate(v1), visited)
                }
            }
            (_, _) => false,
        }
    }
    pub fn strict_equalities(
        t1: &Vec<Self>,
        t2: &Vec<Self>,
        visited: HashSet<*mut ParametricType>,
    ) -> bool {
        t1.len() == t2.len()
            && t1
                .iter()
                .zip(t2.iter())
                .all(|(t1, t2)| Type::strict_equality(t1, t2, visited.clone()))
    }
    pub fn strict_equalities_option(
        t1: &Vec<Option<Self>>,
        t2: &Vec<Option<Self>>,
        visited: HashSet<*mut ParametricType>,
    ) -> bool {
        t1.len() == t2.len()
            && t1.iter().zip(t2.iter()).all(|(t1, t2)| match (t1, t2) {
                (None, None) => true,
                (Some(t1), Some(t2)) => Type::strict_equality(t1, t2, visited.clone()),
                _ => false,
            })
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TypeAtomic(pub AtomicTypeEnum);

impl TypeAtomic {
    fn strict_equality(&self, other: &Self, _: Visited) -> bool {
        self == other
    }
}

#[derive(Clone, Eq, Hash)]
pub struct TypeUnion {
    pub id: Id,
    pub variants: Vec<Option<Type>>,
}

strict_partial_eq!(TypeUnion);

impl TypeUnion {
    fn strict_equality(&self, other: &Self, visited: Visited) -> bool {
        self.id == other.id
            && Type::strict_equalities_option(&self.variants, &other.variants, visited)
    }
}

#[derive(Clone, Eq)]
pub struct TypeInstantiation {
    pub reference: Rc<RefCell<ParametricType>>,
    pub instances: Vec<Type>,
}

strict_partial_eq!(TypeInstantiation);

impl TypeInstantiation {
    fn strict_equality(&self, other: &Self, visited: Visited) -> bool {
        Type::strict_equality(&self.clone().into(), &other.clone().into(), visited)
    }
}

impl Hash for TypeInstantiation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.reference.as_ptr().hash(state);
        self.instances.hash(state);
    }
}

#[derive(Clone, Eq, Hash)]
pub struct TypeTuple(pub Vec<Type>);

strict_partial_eq!(TypeTuple);

impl TypeTuple {
    fn strict_equality(&self, other: &Self, visited: Visited) -> bool {
        Type::strict_equalities(&self.0, &other.0, visited)
    }
}

#[derive(Clone, Eq, Hash)]
pub struct TypeFn(pub Vec<Type>, pub Box<Type>);

strict_partial_eq!(TypeFn);

impl TypeFn {
    fn strict_equality(&self, other: &Self, visited: Visited) -> bool {
        Type::strict_equalities(&self.0, &other.0, visited.clone())
            && Type::strict_equality(&self.1, &other.1, visited)
    }
}

#[derive(Clone, Eq)]
pub struct TypeVariable(pub Rc<RefCell<Option<Type>>>);

strict_partial_eq!(TypeVariable);

impl TypeVariable {
    fn strict_equality(&self, other: &Self, _: Visited) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl Hash for TypeVariable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

pub const TYPE_INT: Type = Type::TypeAtomic(TypeAtomic(AtomicTypeEnum::INT));
pub const TYPE_BOOL: Type = Type::TypeAtomic(TypeAtomic(AtomicTypeEnum::BOOL));
pub const TYPE_UNIT: Type = Type::TypeTuple(TypeTuple(Vec::new()));

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::TypeAtomic(TypeAtomic(atomic_type)) => write!(f, "TypeAtomic({:?})", atomic_type),
            Type::TypeUnion(TypeUnion { id, variants }) => {
                write!(f, "TypeUnion({:?},{:?})", id, variants.iter().collect_vec())
            }
            Type::TypeInstantiation(TypeInstantiation {
                reference,
                instances,
            }) => {
                write!(
                    f,
                    "TypeInstantiation({:p},{:?})",
                    Rc::as_ptr(reference),
                    instances
                )
            }
            Type::TypeTuple(TypeTuple(types)) => write!(f, "TypeTuple({:?})", types),
            Type::TypeFn(TypeFn(argument_type, return_type)) => {
                write!(f, "TypeFn({:?},{:?})", argument_type, return_type)
            }
            Type::TypeVariable(TypeVariable(idx)) => write!(f, "TypeVariable({:?})", idx.as_ptr()),
        }
    }
}

#[derive(Eq, Clone)]
pub struct Variable(pub Rc<RefCell<()>>);

impl fmt::Debug for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Variable").field(&self.0.as_ptr()).finish()
    }
}

impl Variable {
    pub fn new() -> Self {
        Variable(Rc::new(RefCell::new(())))
    }
}

impl Hash for Variable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedTuple {
    pub expressions: Vec<TypedExpression>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedAccess {
    pub variable: TypedVariable,
    pub parameters: Vec<Type>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedElementAccess {
    pub expression: Box<TypedExpression>,
    pub index: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedIf {
    pub condition: Box<TypedExpression>,
    pub true_block: TypedBlock,
    pub false_block: TypedBlock,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatchItem {
    pub type_idx: usize,
    pub assignee: Option<TypedVariable>,
}

impl TypedMatchItem {
    fn instantiate(&self) -> Self {
        TypedMatchItem {
            type_idx: self.type_idx,
            assignee: self
                .assignee
                .as_ref()
                .map(|assignee| assignee.instantiate()),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatchBlock {
    pub matches: Vec<TypedMatchItem>,
    pub block: TypedBlock,
}

impl TypedMatchBlock {
    fn instantiate(&self) -> Self {
        TypedMatchBlock {
            matches: self
                .matches
                .iter()
                .map(|match_| match_.instantiate())
                .collect(),
            block: self.block.instantiate(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedMatch {
    pub subject: Box<TypedExpression>,
    pub blocks: Vec<TypedMatchBlock>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedLambdaDef {
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
    pub idx: usize,
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
    TypedLambdaDef(TypedLambdaDef),
    TypedFunctionCall(TypedFunctionCall),
    TypedConstructorCall(TypedConstructorCall),
}

impl TypedExpression {
    pub fn type_(&self) -> Type {
        let type_ = match self {
            Self::Integer(_) => TYPE_INT,
            Self::Boolean(_) => TYPE_BOOL,
            Self::TypedTuple(TypedTuple { expressions }) => {
                TypeTuple(Self::types(expressions)).into()
            }
            Self::TypedAccess(TypedAccess {
                variable: TypedVariable { variable: _, type_ },
                parameters,
            }) => type_.instantiate(parameters),
            Self::TypedElementAccess(TypedElementAccess { expression, index }) => {
                if let Type::TypeTuple(TypeTuple(types)) = expression.type_() {
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
            Self::TypedLambdaDef(TypedLambdaDef {
                parameters,
                return_type,
                body: _,
            }) => TypeFn(
                parameters
                    .iter()
                    .map(|parameter| parameter.type_.type_.clone())
                    .collect_vec(),
                return_type.clone(),
            )
            .into(),
            Self::TypedFunctionCall(TypedFunctionCall {
                function,
                arguments: _,
            }) => {
                let Type::TypeFn(TypeFn(_, return_type)) = function.type_() else {
                    panic!("Function does not have function type.")
                };
                *return_type
            }
            Self::TypedConstructorCall(TypedConstructorCall {
                output_type,
                idx: _,
                arguments: _,
            }) => output_type.clone(),
            Self::TypedMatch(TypedMatch { subject: _, blocks }) => {
                let Some(block) = blocks.first() else {
                    panic!("Match block with no blocks.")
                };
                block.block.type_()
            }
        };
        let type_ = if let Type::TypeInstantiation(TypeInstantiation {
            reference: r,
            instances: t,
        }) = type_
        {
            r.borrow().instantiate(&t)
        } else {
            type_
        };
        type_
    }
    pub fn types(expressions: &Vec<Self>) -> Vec<Type> {
        expressions.iter().map(Self::type_).collect_vec()
    }
    fn instantiate(&self) -> TypedExpression {
        match &self {
            Self::Boolean(_) | Self::Integer(_) => self.clone(),
            Self::TypedTuple(TypedTuple { expressions }) => TypedTuple {
                expressions: (Self::instantiate_expressions(expressions)),
            }
            .into(),
            Self::TypedAccess(TypedAccess {
                variable,
                parameters,
            }) => TypedAccess {
                variable: variable.instantiate(),
                parameters: (Type::instantiate_types(parameters)),
            }
            .into(),
            Self::TypedElementAccess(TypedElementAccess { expression, index }) => {
                TypedElementAccess {
                    expression: Box::new(expression.instantiate()),
                    index: *index,
                }
                .into()
            }
            Self::TypedIf(TypedIf {
                condition,
                true_block,
                false_block,
            }) => TypedIf {
                condition: Box::new(condition.instantiate()),
                true_block: true_block.instantiate(),
                false_block: false_block.instantiate(),
            }
            .into(),
            Self::TypedMatch(TypedMatch { subject, blocks }) => TypedMatch {
                subject: Box::new(subject.instantiate()),
                blocks: blocks.iter().map(|block| block.instantiate()).collect(),
            }
            .into(),
            Self::TypedLambdaDef(TypedLambdaDef {
                parameters,
                return_type,
                body,
            }) => TypedLambdaDef {
                parameters: parameters
                    .iter()
                    .map(|parameter| parameter.instantiate())
                    .collect(),
                return_type: Box::new(return_type.instantiate()),
                body: body.instantiate(),
            }
            .into(),
            Self::TypedFunctionCall(TypedFunctionCall {
                function,
                arguments,
            }) => TypedFunctionCall {
                function: Box::new(function.instantiate()),
                arguments: (Self::instantiate_expressions(arguments)),
            }
            .into(),
            Self::TypedConstructorCall(TypedConstructorCall {
                idx,
                output_type,
                arguments,
            }) => TypedConstructorCall {
                idx: *idx,
                output_type: output_type.instantiate(),
                arguments: Self::instantiate_expressions(arguments),
            }
            .into(),
        }
    }

    fn instantiate_expressions(expressions: &Vec<TypedExpression>) -> Vec<TypedExpression> {
        expressions
            .iter()
            .map(|expression| expression.instantiate())
            .collect_vec()
    }

    pub fn equal(e1: &Self, e2: &Self) -> bool {
        match (e1, e2) {
            (TypedExpression::Integer(i1), TypedExpression::Integer(i2)) => i1 == i2,
            (TypedExpression::Boolean(b1), TypedExpression::Boolean(b2)) => b1 == b2,
            (
                TypedExpression::TypedTuple(TypedTuple { expressions: e1 }),
                TypedExpression::TypedTuple(TypedTuple { expressions: e2 }),
            ) => Self::equal_expressions(e1, e2),
            (
                TypedExpression::TypedAccess(TypedAccess {
                    variable: _,
                    parameters: p1,
                }),
                TypedExpression::TypedAccess(TypedAccess {
                    variable: _,
                    parameters: p2,
                }),
            ) => p1.len() == p2.len(),
            (
                TypedExpression::TypedElementAccess(TypedElementAccess {
                    expression: e1,
                    index: i1,
                }),
                TypedExpression::TypedElementAccess(TypedElementAccess {
                    expression: e2,
                    index: i2,
                }),
            ) => i1 == i2 && Self::equal(&*e1, &*e2),
            (
                TypedExpression::TypedIf(TypedIf {
                    condition: c1,
                    true_block: t1,
                    false_block: f1,
                }),
                TypedExpression::TypedIf(TypedIf {
                    condition: c2,
                    true_block: t2,
                    false_block: f2,
                }),
            ) => {
                Self::equal(&*c1, &*c2) && Self::equal_blocks(t1, t2) && Self::equal_blocks(f1, f2)
            }
            (
                TypedExpression::TypedMatch(TypedMatch {
                    subject: s1,
                    blocks: b1,
                }),
                TypedExpression::TypedMatch(TypedMatch {
                    subject: s2,
                    blocks: b2,
                }),
            ) => {
                Self::equal(&*s1, &*s2)
                    && b1.len() == b2.len()
                    && b1.iter().zip_eq(b2.iter()).all(|(b1, b2)| {
                        b1.matches.len() == b2.matches.len()
                            && Self::equal_blocks(&b1.block, &b2.block)
                    })
            }
            (
                TypedExpression::TypedLambdaDef(TypedLambdaDef {
                    parameters: p1,
                    return_type: r1,
                    body: b1,
                }),
                TypedExpression::TypedLambdaDef(TypedLambdaDef {
                    parameters: p2,
                    return_type: r2,
                    body: b2,
                }),
            ) => p1.len() == p2.len() && r1 == r2 && Self::equal_blocks(b1, b2),
            (
                TypedExpression::TypedFunctionCall(TypedFunctionCall {
                    function: f1,
                    arguments: a1,
                }),
                TypedExpression::TypedFunctionCall(TypedFunctionCall {
                    function: f2,
                    arguments: a2,
                }),
            ) => Self::equal(&*f1, &*f2) && Self::equal_expressions(a1, a2),
            (
                TypedExpression::TypedConstructorCall(TypedConstructorCall {
                    idx: i1,
                    output_type: o1,
                    arguments: a1,
                }),
                TypedExpression::TypedConstructorCall(TypedConstructorCall {
                    idx: i2,
                    output_type: o2,
                    arguments: a2,
                }),
            ) => i1 == i2 && o1 == o2 && Self::equal_expressions(a1, a2),
            _ => false,
        }
    }
    pub fn equal_expressions(e1: &Vec<Self>, e2: &Vec<Self>) -> bool {
        e1.len() == e2.len()
            && e1
                .iter()
                .zip_eq(e2.iter())
                .all(|(e1, e2)| Self::equal(e1, e2))
    }
    pub fn equal_blocks(b1: &TypedBlock, b2: &TypedBlock) -> bool {
        Self::equal_statements(&b1.statements, &b2.statements)
            && Self::equal(&b1.expression, &b2.expression)
    }
    pub fn equal_statements(s1: &Vec<TypedStatement>, s2: &Vec<TypedStatement>) -> bool {
        s1.len() == s2.len()
            && s1
                .iter()
                .zip_eq(s2.iter())
                .all(|(s1, s2)| Self::equal_statement(s1, s2))
    }
    pub fn equal_statement(s1: &TypedStatement, s2: &TypedStatement) -> bool {
        match (s1, s2) {
            (
                TypedStatement::TypedAssignment(TypedAssignment {
                    variable: v1,
                    expression: e1,
                }),
                TypedStatement::TypedAssignment(TypedAssignment {
                    variable: v2,
                    expression: e2,
                }),
            ) => v1.type_ == v2.type_ && Self::equal(&e1.expression, &e2.expression),
            (TypedStatement::TypedFnDef(_), TypedStatement::TypedFnDef(_)) => todo!(),
            _ => false,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ParametricExpression {
    pub expression: TypedExpression,
    pub parameters: Vec<(Id, Rc<RefCell<Option<Type>>>)>,
}

impl ParametricExpression {
    pub fn instantiate(&self, type_variables: &Vec<Type>) -> TypedExpression {
        for ((_, parameter), variable) in self.parameters.iter().zip_eq(type_variables) {
            *parameter.borrow_mut() = Some(variable.clone());
        }
        let expression = self.expression.instantiate();
        for (_, parameter) in &self.parameters {
            *parameter.borrow_mut() = None;
        }
        expression
    }
}

impl From<TypedExpression> for ParametricExpression {
    fn from(value: TypedExpression) -> Self {
        ParametricExpression {
            expression: value,
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, FromVariants)]
pub enum TypedStatement {
    TypedAssignment(TypedAssignment),
    TypedFnDef(TypedFnDef),
}

impl TypedStatement {
    pub fn instantiate_statements(statements: &Vec<Self>) -> Vec<Self> {
        statements
            .iter()
            .map(|statement| statement.instantiate())
            .collect()
    }
    pub fn instantiate(&self) -> Self {
        match self {
            TypedStatement::TypedAssignment(typed_assignment) => {
                typed_assignment.instantiate().into()
            }
            TypedStatement::TypedFnDef(_) => todo!(),
        }
    }
    pub fn variable(&self) -> TypedVariable {
        match self {
            TypedStatement::TypedAssignment(TypedAssignment {
                variable,
                expression: _,
            })
            | TypedStatement::TypedFnDef(TypedFnDef {
                variable,
                parameters: _,
                fn_: _,
            }) => variable.clone(),
        }
    }
    pub fn parameters(&self) -> Vec<Rc<RefCell<Option<Type>>>> {
        match self {
            TypedStatement::TypedAssignment(TypedAssignment {
                variable,
                expression: _,
            })
            | TypedStatement::TypedFnDef(TypedFnDef {
                variable,
                parameters: _,
                fn_: _,
            }) => variable.type_.parameters.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedAssignment {
    pub variable: TypedVariable,
    pub expression: ParametricExpression,
}

impl TypedAssignment {
    pub fn instantiate(&self) -> Self {
        TypedAssignment {
            expression: ParametricExpression {
                expression: self.expression.expression.instantiate(),
                parameters: self.expression.parameters.clone(),
            },
            variable: TypedVariable {
                variable: self.variable.variable.clone(),
                type_: ParametricType {
                    type_: self.variable.type_.type_.instantiate(),
                    parameters: self.variable.type_.parameters.clone(),
                },
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedFnDef {
    pub variable: TypedVariable,
    pub parameters: Vec<(Id, Rc<RefCell<Option<Type>>>)>,
    pub fn_: TypedLambdaDef,
}

impl TypedFnDef {
    pub fn instantiate(&self) -> Self {
        TypedFnDef {
            fn_: {
                let TypedExpression::TypedLambdaDef(fn_) =
                    TypedExpression::from(self.fn_.clone()).instantiate()
                else {
                    panic!("Typed function changed form")
                };
                fn_
            },
            variable: TypedVariable {
                variable: self.variable.variable.clone(),
                type_: ParametricType {
                    type_: self.variable.type_.type_.instantiate(),
                    parameters: self.variable.type_.parameters.clone(),
                },
            },
            parameters: self.parameters.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedBlock {
    pub statements: Vec<TypedStatement>,
    pub expression: Box<TypedExpression>,
}

impl TypedBlock {
    pub fn type_(&self) -> Type {
        return self.expression.type_();
    }
    pub fn instantiate(&self) -> TypedBlock {
        TypedBlock {
            statements: TypedStatement::instantiate_statements(&self.statements),
            expression: Box::new((*self.expression).instantiate()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TypedProgram {
    pub type_definitions: TypeDefinitions,
    pub main: TypedVariable,
    pub statements: Vec<TypedStatement>,
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
        index: usize,
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
    MainFunctionReturnsFunction {
        type_: Type,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorType {
    pub type_: Rc<RefCell<ParametricType>>,
    pub index: usize,
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
pub struct TypeDefinitions(pub HashMap<K, V>);

impl TypeDefinitions {
    pub fn new() -> Self {
        TypeDefinitions(HashMap::new())
    }
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
            (Type::TypeAtomic(a1), Type::TypeAtomic(a2)) => a1 == a2,
            (
                Type::TypeUnion(TypeUnion {
                    id: i1,
                    variants: v1,
                }),
                Type::TypeUnion(TypeUnion {
                    id: i2,
                    variants: v2,
                }),
            ) => {
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
            (
                Type::TypeInstantiation(TypeInstantiation {
                    reference: t1,
                    instances: i1,
                }),
                Type::TypeInstantiation(TypeInstantiation {
                    reference: t2,
                    instances: i2,
                }),
            ) => {
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
            (Type::TypeTuple(TypeTuple(t1)), Type::TypeTuple(TypeTuple(t2))) => {
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
            (Type::TypeFn(TypeFn(a1, r1)), Type::TypeFn(TypeFn(a2, r2))) => {
                TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    self_generics_index,
                    other_generics_index,
                    &TypeTuple(a1.clone()).into(),
                    &TypeTuple(a2.clone()).into(),
                ) && TypeDefinitions::type_equality(
                    self_references_index,
                    other_references_index,
                    self_generics_index,
                    other_generics_index,
                    r1,
                    r2,
                )
            }
            (Type::TypeVariable(TypeVariable(r1)), Type::TypeVariable(TypeVariable(r2))) => {
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
            Type::TypeUnion(TypeUnion { id, variants }) => {
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
            Type::TypeInstantiation(TypeInstantiation {
                reference: rc,
                instances,
            }) => {
                write!(
                    f,
                    "Instantiation({}, {:?})",
                    references_index
                        .get(&rc.as_ptr())
                        .unwrap_or(&Id::from("unknown")),
                    instances
                        .into_iter()
                        .map(|type_| DebugTypeWrapper(type_, references_index.clone()))
                        .collect_vec()
                )
            }
            Type::TypeTuple(TypeTuple(types)) => {
                write!(
                    f,
                    "Tuple({:?})",
                    types
                        .into_iter()
                        .map(|type_| DebugTypeWrapper(type_, references_index.clone()))
                        .collect_vec()
                )
            }
            Type::TypeFn(TypeFn(argument_types, return_type)) => {
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

pub type TypeContext = HashMap<Id, TypedVariable>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Boolean, Integer};

    use test_case::test_case;

    #[test_case(
        ParametricExpression{
            parameters: vec![(Id::from("T"), Rc::new(RefCell::new(None)))],
            expression: Boolean { value: true }.into()
        },
        vec![TYPE_INT],
        Boolean { value: true }.into();
        "boolean expression"
    )]
    #[test_case(
        ParametricExpression{
            parameters: Vec::new(),
            expression: Integer { value: 8 }.into()
        },
        Vec::new(),
        Integer { value: 8 }.into();
        "integer expression"
    )]
    #[test_case(
        {
            let left = Rc::new(RefCell::new(None));
            let right = Rc::new(RefCell::new(None));
            let arg0 = TypedVariable::from(Type::TypeVariable(TypeVariable(left.clone())));
            let arg1 = TypedVariable::from(Type::TypeVariable(TypeVariable(right.clone())));
            ParametricExpression{
                parameters: vec![(Id::from("T"), left.clone()), (Id::from("U"), right.clone())],
                expression: TypedLambdaDef{
                    parameters: vec![arg0.clone(), arg1.clone()],
                    body: TypedBlock{
                        statements: Vec::new(),
                        expression: Box::new(TypedTuple{
                            expressions: vec![
                                TypedAccess{
                                    variable: arg0.into(),
                                    parameters: Vec::new()
                                }.into(),
                                TypedAccess{
                                    variable: arg1.into(),
                                    parameters: Vec::new()
                                }.into(),
                            ]
                        }.into())
                    },
                    return_type: Box::new(TypeTuple(vec![TypeVariable(left.clone()).into(), TypeVariable(right.clone()).into()]).into())
                }.into()
            }
        },
        vec![TYPE_INT, TYPE_BOOL],
        {
            let arg0 = TypedVariable::from(TYPE_INT);
            let arg1 = TypedVariable::from(TYPE_BOOL);
            TypedLambdaDef{
                parameters: vec![arg0.clone(), arg1.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedTuple{
                        expressions: vec![
                            TypedAccess{
                                variable: arg0.into(),
                                parameters: Vec::new()
                            }.into(),
                            TypedAccess{
                                variable: arg1.into(),
                                parameters: Vec::new()
                            }.into(),
                        ]
                    }.into())
                },
                return_type: Box::new(TypeTuple(vec![TYPE_INT, TYPE_BOOL]).into())
            }.into()
        };
        "tuple function expression"
    )]
    #[test_case(
        {
            let a = Rc::new(RefCell::new(None));
            let b = Rc::new(RefCell::new(None));
            let arg0 = TypedVariable::from(Type::from(TypeFn(vec![TypeVariable(a.clone()).into()],Box::new(TypeVariable(b.clone()).into()))));
            let arg1 = TypedVariable::from(Type::from(TypeVariable(a.clone())));
            let variable = TypedVariable::from(Type::from(TypeVariable(b.clone())));
            ParametricExpression{
                parameters: vec![(Id::from("F"), a.clone()), (Id::from("T"), b.clone())],
                expression: TypedLambdaDef{
                    parameters: vec![arg0.clone(), arg1.clone()],
                    body: TypedBlock{
                        statements: vec![
                            TypedAssignment{
                                variable: variable.clone(),
                                expression: TypedExpression::from(TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: arg0.into(),
                                            parameters: Vec::new()
                                        }.into(),
                                    ),
                                    arguments: vec![
                                        TypedAccess{
                                            variable: arg1.into(),
                                            parameters: Vec::new()
                                        }.into(),
                                    ]
                                }).into()
                            }.into()
                        ],
                        expression: Box::new(TypedAccess{
                            variable: variable.into(),
                            parameters: Vec::new()
                        }.into())
                    },
                    return_type: Box::new(TypeVariable(b.clone()).into())
                }.into()
            }
        },
        vec![TYPE_INT, TYPE_BOOL],
        {
            let arg0 = TypedVariable::from(Type::from(TypeFn(vec![TYPE_INT],Box::new(TYPE_BOOL))));
            let arg1 = TypedVariable::from(TYPE_INT);
            let variable = TypedVariable::from(TYPE_BOOL);
            TypedLambdaDef{
                parameters: vec![arg0.clone(), arg1.clone()],
                body: TypedBlock{
                    statements: vec![
                        TypedAssignment{
                            variable: variable.clone(),
                            expression: TypedExpression::from(TypedFunctionCall{
                                function: Box::new(
                                    TypedAccess{
                                        variable: arg0.into(),
                                        parameters: Vec::new()
                                    }.into(),
                                ),
                                arguments: vec![
                                    TypedAccess{
                                        variable: arg1.into(),
                                        parameters: Vec::new()
                                    }.into(),
                                ]
                            }).into()
                        }.into()
                    ],
                    expression: Box::new(TypedAccess{
                        variable: variable.into(),
                        parameters: Vec::new()
                    }.into())
                },
                return_type: Box::new(TYPE_BOOL)
            }.into()
        };
        "function application expression"
    )]
    #[test_case(
        {
            let parameter = Rc::new(RefCell::new(None));
            let arg0 = TypedVariable::from(TYPE_BOOL);
            let arg1 = TypedVariable::from(Type::from(TypeVariable(parameter.clone())));
            let arg2 = TypedVariable::from(Type::from(TypeVariable(parameter.clone())));
            ParametricExpression{
                parameters: vec![(Id::from("T"), parameter.clone())],
                expression: TypedLambdaDef{
                    parameters: vec![arg0.clone(), arg1.clone(), arg2.clone()],
                    body: TypedBlock{
                        statements: Vec::new(),
                        expression: Box::new(TypedIf{
                            condition: Box::new(
                                TypedAccess{
                                    variable: arg0.into(),
                                    parameters: Vec::new()
                                }.into(),
                            ),
                            true_block: TypedBlock {
                                statements: Vec::new(),
                                expression: Box::new(
                                    TypedAccess{
                                        variable: arg1.into(),
                                        parameters: Vec::new()
                                    }.into(),
                                )
                            },
                            false_block: TypedBlock {
                                statements: Vec::new(),
                                expression: Box::new(
                                    TypedAccess{
                                        variable: arg2.into(),
                                        parameters: Vec::new()
                                    }.into(),
                                )
                            },
                        }.into())
                    },
                    return_type: Box::new(TypeVariable(parameter.clone()).into())
                }.into()
            }
        },
        vec![TYPE_BOOL],
        {
            let arg0 = TypedVariable::from(TYPE_BOOL);
            let arg1 = TypedVariable::from(TYPE_BOOL);
            let arg2 = TypedVariable::from(TYPE_BOOL);
            TypedLambdaDef{
                parameters: vec![arg0.clone(), arg1.clone(), arg2.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedIf{
                        condition: Box::new(
                            TypedAccess{
                                variable: arg0.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        true_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                TypedAccess{
                                    variable: arg1.into(),
                                    parameters: Vec::new()
                                }.into(),
                            )
                        },
                        false_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                TypedAccess{
                                    variable: arg2.into(),
                                    parameters: Vec::new()
                                }.into(),
                            )
                        },
                    }.into())
                },
                return_type: Box::new(TYPE_BOOL)
            }.into()

        };
        "if statement expression"
    )]
    #[test_case(
        {
            let left = Rc::new(RefCell::new(None));
            let right = Rc::new(RefCell::new(None));
            let arg = TypedVariable::from(Type::from(TypeVariable(left.clone())));
            let variable = TypedVariable::from(Type::from(TypeUnion{id: Id::from("Either"), variants: vec![Some(TypeVariable(left.clone()).into()), Some(TypeVariable(right.clone()).into())]}));
            let subvariable = TypedVariable::from(Type::from(TypeVariable(left.clone())));
            ParametricExpression{
                parameters: vec![(Id::from("T"), left.clone()),(Id::from("U"), right.clone())],
                expression: TypedLambdaDef{
                    parameters: vec![arg.clone()],
                    body: TypedBlock{
                        statements: vec![
                            TypedAssignment{
                                variable: variable.clone(),
                                expression: TypedExpression::from(TypedConstructorCall{
                                    idx: 0,
                                    output_type: variable.type_.type_.clone(),
                                    arguments: vec![
                                        TypedAccess{
                                            variable: arg.clone().into(),
                                            parameters: Vec::new()
                                        }.into(),
                                    ],
                                }).into()
                            }.into()
                        ],
                        expression: Box::new(TypedMatch{
                            subject: Box::new(
                                TypedAccess{
                                    variable: variable.into(),
                                    parameters: Vec::new()
                                }.into(),
                            ),
                            blocks: vec![
                                TypedMatchBlock {
                                    matches: vec![
                                        TypedMatchItem {
                                            type_idx: 0,
                                            assignee: Some(subvariable.clone()),
                                        }
                                    ],
                                    block: TypedBlock {
                                        statements: Vec::new(),
                                        expression: Box::new(
                                            TypedAccess{
                                                variable: subvariable.into(),
                                                parameters: Vec::new()
                                            }.into(),
                                        )
                                    }
                                },
                                TypedMatchBlock {
                                    matches: vec![
                                        TypedMatchItem {
                                            type_idx: 1,
                                            assignee: None,
                                        }
                                    ],
                                    block: TypedBlock {
                                        statements: Vec::new(),
                                        expression: Box::new(
                                            TypedAccess{
                                                variable: arg.into(),
                                                parameters: Vec::new()
                                            }.into(),
                                        )
                                    }
                                },
                            ]
                        }.into())
                    },
                    return_type: Box::new(TypeVariable(left.clone()).into())
                }.into()
            }
        },
        vec![TYPE_BOOL, TYPE_UNIT],
        {

            let arg = TypedVariable::from(TYPE_BOOL);
            let variable = TypedVariable::from(Type::from(TypeUnion{id: Id::from("Either"), variants: vec![Some(TYPE_BOOL), Some(TYPE_UNIT)]}));
            let subvariable = TypedVariable::from(TYPE_BOOL);
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: vec![
                        TypedAssignment{
                            variable: variable.clone(),
                            expression: TypedExpression::from(TypedConstructorCall{
                                idx: 0,
                                output_type: variable.type_.type_.clone(),
                                arguments: vec![
                                    TypedAccess{
                                        variable: arg.clone().into(),
                                        parameters: Vec::new()
                                    }.into(),
                                ],
                            }).into()
                        }.into()
                    ],
                    expression: Box::new(TypedMatch{
                        subject: Box::new(
                            TypedAccess{
                                variable: variable.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        blocks: vec![
                            TypedMatchBlock {
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 0,
                                        assignee: Some(subvariable.clone()),
                                    }
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        TypedAccess{
                                            variable: subvariable.into(),
                                            parameters: Vec::new()
                                        }.into(),
                                    )
                                }
                            },
                            TypedMatchBlock {
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 1,
                                        assignee: None,
                                    }
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        TypedAccess{
                                            variable: arg.into(),
                                            parameters: Vec::new()
                                        }.into(),
                                    )
                                }
                            },
                        ]
                    }.into())
                },
                return_type: Box::new(TYPE_BOOL)
            }.into()
        };
        "union type expression"
    )]
    fn test_instantiate(
        expression: ParametricExpression,
        types: Vec<Type>,
        expected: TypedExpression,
    ) {
        assert!(TypedExpression::equal(
            &expression.instantiate(&types),
            &expected
        ));
    }
}
