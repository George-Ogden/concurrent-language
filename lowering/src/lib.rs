mod copy_propagation;
mod expression_equality_checker;
mod fn_inst;
mod intermediate_nodes;
mod lower;
mod recursive_fn_finder;
mod type_equality_checker;

pub use copy_propagation::CopyPropagator;
pub use expression_equality_checker::ExpressionEqualityChecker;
pub use fn_inst::{FnDefs, FnInst};
pub use intermediate_nodes::*;
pub use lower::Lowerer;
pub use recursive_fn_finder::{RecursiveFnFinder, RecursiveFns};
pub use type_checker::{AtomicTypeEnum, Boolean, Id, Integer, DEFAULT_CONTEXT};
