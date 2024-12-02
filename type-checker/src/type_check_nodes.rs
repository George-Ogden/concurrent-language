use crate::{AtomicTypeEnum, Boolean, Id, Integer};
use from_variants::FromVariants;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::rc::Rc;

#[derive(PartialEq, Clone, Debug)]
pub struct ParametricType {
    pub type_: Type,
    pub num_parameters: u32,
}

impl From<Type> for ParametricType {
    fn from(value: Type) -> Self {
        ParametricType {
            type_: value,
            num_parameters: 0,
        }
    }
}

impl ParametricType {
    pub fn new() -> Self {
        ParametricType {
            type_: Type::new(),
            num_parameters: 0,
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum Type {
    Atomic(AtomicTypeEnum),
    Union(HashMap<Id, Option<Type>>),
    Instantiation(Rc<RefCell<ParametricType>>, Vec<Type>),
    Tuple(Vec<Type>),
    Function(Box<Type>, Box<Type>),
    Variable(u32),
}

impl Type {
    pub fn new() -> Self {
        Type::Tuple(Vec::new())
    }
}

pub const TYPE_INT: Type = Type::Atomic(AtomicTypeEnum::INT);
pub const TYPE_BOOL: Type = Type::Atomic(AtomicTypeEnum::BOOL);

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Atomic(atomic_type) => write!(f, "Atomic({:?})", atomic_type),
            Type::Union(variants) => write!(f, "Union({:?})", variants.iter().collect_vec()),
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
            Type::Variable(idx) => write!(f, "Variable({:?})", idx),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedTuple {
    pub expressions: Vec<TypedExpression>,
}

#[derive(Debug, PartialEq, Clone, FromVariants)]
pub enum TypedExpression {
    Integer(Integer),
    Boolean(Boolean),
    TypedTuple(TypedTuple),
}

impl TypedExpression {
    pub fn type_(&self) -> Type {
        match self {
            Self::Integer(_) => TYPE_INT,
            Self::Boolean(_) => TYPE_BOOL,
            Self::TypedTuple(TypedTuple { expressions }) => {
                Type::Tuple(expressions.iter().map(TypedExpression::type_).collect_vec())
            }
            _ => todo!(),
        }
    }
}
