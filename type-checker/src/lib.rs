mod ast_nodes;
mod type_check;
mod type_check_nodes;
mod utils;

use ast_nodes::*;
pub use ast_nodes::{AtomicTypeEnum, Boolean, Integer, Program};
pub use type_check::TypeChecker;
pub use type_check_nodes::{ParametricType, Type, TypedExpression, TypedVariable, Variable};
