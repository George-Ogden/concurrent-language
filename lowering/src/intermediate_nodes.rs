use std::{cell::RefCell, rc::Rc};

use from_variants::FromVariants;
use type_checker::{Boolean, Integer};

use crate::{AtomicType, Name};

#[derive(Clone, Debug, PartialEq, FromVariants)]
pub enum IntermediateType {
    AtomicType(AtomicType),
    IntermediateTupleType(IntermediateTupleType),
    IntermediateFnType(IntermediateFnType),
    IntermediateUnionType(IntermediateUnionType),
    IntermediateReferenceType(Rc<RefCell<IntermediateType>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateTupleType(pub Vec<IntermediateType>);

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateFnType(pub Vec<IntermediateType>, pub Box<IntermediateType>);

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateUnionType(Rc<RefCell<Vec<Option<IntermediateType>>>>);

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDef(pub Rc<RefCell<Vec<Option<IntermediateType>>>>);

#[derive(Clone, Debug, PartialEq)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(IntermediateMemory),
}

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, IntermediateType),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateMemory(Rc<RefCell<IntermediateExpression>>);

#[derive(Clone, Debug, PartialEq, FromVariants)]
pub enum IntermediateExpression {
    IntermediateArgument(IntermediateType),
    IntermediateValue(IntermediateValue),
    IntermediateElementAccess(IntermediateElementAccess),
    IntermediateTupleExpression(IntermediateTupleExpression),
    IntermediateFnCall(IntermediateFnCall),
    IntermediateCtorCall(IntermediateCtorCall),
    IntermediateFnDef(IntermediateFnDef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateElementAccess {
    pub value: IntermediateValue,
    pub idx: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateTupleExpression(pub Vec<IntermediateValue>);

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateFnCall {
    pub fn_: IntermediateValue,
    pub args: Vec<IntermediateValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateCtorCall {
    pub idx: usize,
    pub data: Rc<RefCell<Vec<Option<IntermediateType>>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateAssignment(pub IntermediateMemory);

#[derive(Clone, Debug, PartialEq)]
pub struct IntermediateFnDef {
    argument_types: Vec<IntermediateType>,
    statements: Vec<IntermediateStatement>,
    return_value: IntermediateValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IntermediateStatement {}
