use from_variants::FromVariants;
use type_checker::AtomicTypeEnum;

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
pub struct UnionType(pub String, pub Vec<MachineType>);
