use crate::{AtomicTypeEnum, Block, Boolean, Id, Integer};
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

#[derive(Clone)]
pub enum Type {
    Atomic(AtomicTypeEnum),
    Union(HashMap<Id, Option<Type>>),
    Instantiation(Rc<RefCell<ParametricType>>, Vec<Type>),
    Tuple(Vec<Type>),
    Function(Vec<Type>, Box<Type>),
    Variable(u32),
}

impl PartialEq for Type {
    fn eq(&self, other: &Type) -> bool {
        match (self, other) {
            (Self::Instantiation(r1, t1), Self::Instantiation(r2, t2)) if r1 == r2 => t1 == t2,
            (Self::Instantiation(r1, t1), t2) | (t2, Self::Instantiation(r1, t1)) => {
                t2 == &r1.borrow().type_.instantiate(t1)
            }
            (Self::Atomic(a1), Self::Atomic(a2)) => a1 == a2,
            (Self::Union(h1), Self::Union(h2)) => h1 == h2,
            (Self::Tuple(t1), Self::Tuple(t2)) => t1 == t2,
            (Self::Function(a1, r1), Self::Function(a2, r2)) => a1 == a2 && r1 == r2,
            (Self::Variable(u1), Self::Variable(h2)) => u1 == h2,
            _ => false,
        }
    }
}

impl Type {
    pub fn new() -> Self {
        Type::Tuple(Vec::new())
    }
    pub fn instantiate_types(types: &Vec<Self>, type_variables: &Vec<Type>) -> Vec<Type> {
        types
            .iter()
            .map(|type_| type_.instantiate(type_variables))
            .collect_vec()
    }
    pub fn instantiate(&self, type_variables: &Vec<Type>) -> Type {
        match self {
            Self::Atomic(_) => self.clone(),
            Self::Tuple(types) => Type::Tuple(Type::instantiate_types(types, type_variables)),
            Self::Union(types) => Type::Union(
                types
                    .iter()
                    .map(|(id, type_)| {
                        (
                            id.clone(),
                            type_.clone().map(|type_| type_.instantiate(type_variables)),
                        )
                    })
                    .collect(),
            ),
            Self::Instantiation(parametric_type, types) => Type::Instantiation(
                parametric_type.clone(),
                Self::instantiate_types(types, type_variables),
            ),
            Self::Function(arg_types, return_type) => Type::Function(
                Self::instantiate_types(arg_types, type_variables),
                Box::new(return_type.instantiate(type_variables)),
            ),
            Self::Variable(i) => type_variables[*i as usize].clone(),
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

#[derive(Debug, PartialEq, Clone)]
pub struct TypedVariable {
    pub id: Id,
    pub type_: Type,
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
pub struct PartiallyTypedFunctionDefinition {
    pub parameter_ids: Vec<Id>,
    pub parameter_types: Vec<Type>,
    pub return_type: Box<Type>,
    pub body: Block,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedFunctionDefinition {
    pub parameter_ids: Vec<Id>,
    pub parameter_types: Vec<Type>,
    pub return_type: Box<Type>,
    pub body: TypedBlock,
}

#[derive(Debug, PartialEq, Clone, FromVariants)]
pub enum TypedExpression {
    Integer(Integer),
    Boolean(Boolean),
    TypedTuple(TypedTuple),
    TypedVariable(TypedVariable),
    TypedElementAccess(TypedElementAccess),
    TypedIf(TypedIf),
    PartiallyTypedFunctionDefinition(PartiallyTypedFunctionDefinition),
    TypedFunctionDefinition(TypedFunctionDefinition),
}

impl TypedExpression {
    pub fn type_(&self) -> Type {
        let type_ = match self {
            Self::Integer(_) => TYPE_INT,
            Self::Boolean(_) => TYPE_BOOL,
            Self::TypedTuple(TypedTuple { expressions }) => {
                Type::Tuple(expressions.iter().map(TypedExpression::type_).collect_vec())
            }
            Self::TypedVariable(TypedVariable { id: _, type_ }) => type_.clone(),
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
                parameter_ids: _,
                parameter_types,
                return_type,
                body: _,
            })
            | Self::TypedFunctionDefinition(TypedFunctionDefinition {
                parameter_ids: _,
                parameter_types,
                return_type,
                body: _,
            }) => Type::Function(parameter_types.clone(), return_type.clone()),
            _ => todo!(),
        };
        if let Type::Instantiation(r, t) = type_ {
            r.borrow().type_.instantiate(&t)
        } else {
            type_
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ParametricExpression {
    pub expression: TypedExpression,
    pub num_parameters: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedAssignment {
    pub id: Id,
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

#[derive(Debug, Clone, PartialEq)]
pub enum TypeCheckError {
    DefaultError(String),
    DuplicatedNameError {
        duplicate: Id,
        type_: String,
    },
    InvalidConditionError {
        condition: TypedExpression,
    },
    InvalidAccessError {
        expression: TypedExpression,
    },
    NonMatchingIfBlocksError {
        true_block: TypedBlock,
        false_block: TypedBlock,
    },
    FunctionReturnTypeError {
        return_type: Type,
        body: TypedBlock,
    },
    UnknownTypeError {
        type_name: Id,
        type_names: Vec<Id>,
    },
}
