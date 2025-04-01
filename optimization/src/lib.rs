#![feature(cmp_minmax)]

mod args;
mod dead_code_analysis;
mod inlining;
mod optimizer;
mod redundancy_elimination;
mod refresher;

pub use args::OptimizationArgs;
pub use optimizer::Optimizer;
