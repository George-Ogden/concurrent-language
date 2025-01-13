use core::fmt;
use std::{cell::RefCell, hash::Hash, rc::Rc};

use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

use crate::{AtomicType, Name};

#[derive(Clone, Debug, PartialEq, FromVariants, Eq)]
pub enum IntermediateType {
    AtomicType(AtomicType),
    IntermediateTupleType(IntermediateTupleType),
    IntermediateFnType(IntermediateFnType),
    IntermediateUnionType(IntermediateUnionType),
    IntermediateReferenceType(Rc<RefCell<IntermediateType>>),
}

impl From<AtomicTypeEnum> for IntermediateType {
    fn from(value: AtomicTypeEnum) -> Self {
        Self::AtomicType(AtomicType(value))
    }
}

impl Hash for IntermediateType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleType(pub Vec<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnType(pub Vec<IntermediateType>, pub Box<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateUnionType(pub Vec<Option<IntermediateType>>);

impl Hash for IntermediateUnionType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDef(pub Rc<RefCell<Vec<Option<IntermediateType>>>>);

impl Hash for TypeDef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(IntermediateMemory),
}

impl From<IntermediateExpression> for IntermediateValue {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateValue::IntermediateMemory(value.into())
    }
}

#[derive(Clone, Debug, FromVariants, PartialEq, Eq, Hash)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, IntermediateType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateMemory(Rc<RefCell<IntermediateExpression>>);

impl Hash for IntermediateMemory {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

impl From<IntermediateExpression> for IntermediateMemory {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateMemory(Rc::new(RefCell::new(value)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, FromVariants, Hash)]
pub enum IntermediateExpression {
    IntermediateArgument(IntermediateArgument),
    IntermediateValue(IntermediateValue),
    IntermediateElementAccess(IntermediateElementAccess),
    IntermediateTupleExpression(IntermediateTupleExpression),
    IntermediateFnCall(IntermediateFnCall),
    IntermediateCtorCall(IntermediateCtorCall),
    IntermediateFnDef(IntermediateFnDef),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateArgument(pub IntermediateType);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateElementAccess {
    pub value: IntermediateValue,
    pub idx: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleExpression(pub Vec<IntermediateValue>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnCall {
    pub fn_: IntermediateValue,
    pub args: Vec<IntermediateValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateCtorCall {
    pub idx: usize,
    pub data: Rc<RefCell<Vec<Option<IntermediateType>>>>,
}

impl Hash for IntermediateCtorCall {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.idx.hash(state);
        self.data.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnDef {
    arguments: Vec<IntermediateArgument>,
    statements: Vec<IntermediateStatement>,
    return_value: IntermediateValue,
}

#[derive(Clone, Debug, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateStatement {
    Assignment(IntermediateMemory),
    IntermediateIfStatement(IntermediateIfStatement),
    IntermediateMatchStatement(IntermediateMatchStatement),
}

impl From<IntermediateMemory> for IntermediateStatement {
    fn from(value: IntermediateMemory) -> Self {
        IntermediateStatement::Assignment(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateIfStatement {
    pub condition: IntermediateValue,
    pub branches: (Vec<IntermediateStatement>, Vec<IntermediateStatement>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchStatement {
    pub expression: IntermediateValue,
    pub branches: Vec<IntermediateMatchBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchBranch {
    pub target: Option<IntermediateArgument>,
    pub statements: Vec<IntermediateMatchBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateProgram {
    pub statements: Vec<IntermediateStatement>,
    pub main: IntermediateValue,
}
