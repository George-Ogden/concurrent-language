use from_variants::FromVariants;
use type_checker::AtomicTypeEnum;

#[derive(Debug, Clone, FromVariants)]
pub enum MachineType {
    Atomic(Atomic),
}

#[derive(Debug, Clone)]
pub struct Atomic(pub AtomicTypeEnum);
