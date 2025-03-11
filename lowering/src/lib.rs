mod allocations;
mod intermediate_nodes;
mod lower;

pub use allocations::AllocationOptimizer;
pub use intermediate_nodes::*;
pub use lower::Lowerer;
pub use type_checker::{AtomicTypeEnum, Boolean, Id, Integer, DEFAULT_CONTEXT};
