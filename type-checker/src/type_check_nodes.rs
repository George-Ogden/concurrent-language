use crate::{AtomicTypeEnum, Block, Boolean, Id, Integer, MatchBlock, TypeInstance};
use from_variants::FromVariants;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Debug};
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
pub struct TypedVariable {
    pub variable: Variable,
    pub type_: Rc<RefCell<ParametricType>>,
}

impl From<Rc<RefCell<ParametricType>>> for TypedVariable {
    fn from(value: Rc<RefCell<ParametricType>>) -> Self {
        TypedVariable {
            variable: Rc::new(RefCell::new(())),
            type_: value,
        }
    }
}

impl From<ParametricType> for TypedVariable {
    fn from(value: ParametricType) -> Self {
        TypedVariable::from(Rc::new(RefCell::new(value)))
    }
}

impl From<Type> for TypedVariable {
    fn from(value: Type) -> Self {
        TypedVariable::from(ParametricType::from(value))
    }
}

#[derive(Clone)]
pub enum Type {
    Atomic(AtomicTypeEnum),
    Union(HashMap<Id, Option<Type>>),
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
            Self::Union(types) => Type::Union(
                types
                    .iter()
                    .map(|(id, type_)| (id.clone(), type_.clone().map(|type_| type_.instantiate())))
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
            (Self::Union(h1), Self::Union(h2)) => {
                h1.keys().sorted().collect_vec() == h2.keys().sorted().collect_vec()
                    && h1
                        .keys()
                        .all(|id| Type::option_type_equality(&h1[id], &h2[id], equal_references))
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
    pub variable: Variable,
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
pub struct TypedMatchItem {
    pub type_name: Id,
    pub assignee: Option<Id>,
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
            Self::TypedAccess(TypedAccess { variable: _, type_ }) => type_.clone(),
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
                    .map(|(_, parameter)| parameter.type_.borrow().type_.clone())
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
                    .map(|parameter| parameter.type_.borrow().type_.clone())
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorType {
    pub input_type: Option<Type>,
    pub output_type: Type,
    pub parameters: Vec<Rc<RefCell<Option<Type>>>>,
}

impl ConstructorType {
    pub fn instantiate(&self, type_variables: &Vec<Type>) -> (Option<Type>, Type) {
        for (parameter, variable) in self.parameters.iter().zip_eq(type_variables) {
            *parameter.borrow_mut() = Some(variable.clone());
        }
        let output_type = self.output_type.instantiate();
        let input_type = self.input_type.as_ref().map(Type::instantiate);
        for parameter in &self.parameters {
            *parameter.borrow_mut() = None;
        }
        (input_type, output_type)
    }
}
