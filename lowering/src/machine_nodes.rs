use from_variants::FromVariants;
use type_checker::AtomicTypeEnum;

pub type Name = String;

#[derive(Debug, Clone, FromVariants)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
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
    pub constructors: Vec<(Name, Option<TypeRef>)>,
}

#[derive(Debug, Clone)]
pub enum TypeRef {
    Type(MachineType),
    Name(Name),
}
