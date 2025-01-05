use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Debug, Clone, FromVariants)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
}

#[derive(Debug, Clone)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Debug, Clone)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Debug, Clone)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Debug, Clone)]
pub struct UnionType(pub Vec<Name>);

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

#[derive(Debug, Clone, FromVariants)]
pub enum Value {
    BuiltIn(BuiltIn),
    Store(Store),
}

#[derive(Debug, Clone)]
pub enum Store {
    Memory(Id, MachineType),
    Register(Id, MachineType),
}

impl Store {
    pub fn id(&self) -> Id {
        match &self {
            Self::Memory(id, _) | Self::Register(id, _) => id.clone(),
        }
    }
}

#[derive(Debug, Clone, FromVariants)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, MachineType),
}

#[derive(Debug, Clone, FromVariants)]
pub enum Expression {
    Value(Value),
    ElementAccess(ElementAccess),
}

#[derive(Debug, Clone)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}
