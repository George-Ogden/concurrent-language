#![feature(cmp_minmax)]

mod args;
mod dead_code_analysis;
mod equivalent_expression_elimination;
mod inlining;
mod optimizer;
mod refresher;

pub use args::OptimizationArgs;
pub use optimizer::Optimizer;
