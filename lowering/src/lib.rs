mod allocations;
mod fn_inst;
mod intermediate_nodes;
mod lower;
mod recursive_fn_finder;

pub use allocations::AllocationOptimizer;
pub use fn_inst::{FnDefs, FnInst};
pub use intermediate_nodes::*;
pub use lower::Lowerer;
pub use type_checker::{AtomicTypeEnum, Boolean, Id, Integer, DEFAULT_CONTEXT};
