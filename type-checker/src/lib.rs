mod ast_nodes;
mod prefix;
mod type_check;
mod type_check_nodes;
mod utils;

use ast_nodes::*;
pub use ast_nodes::{AtomicTypeEnum, Boolean, Id, Integer, Program};
pub use type_check::{TypeChecker, DEFAULT_CONTEXT};
pub use type_check_nodes::*;
