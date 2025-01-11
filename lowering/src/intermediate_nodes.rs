use std::{cell::RefCell, rc::Rc};

use from_variants::FromVariants;

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
