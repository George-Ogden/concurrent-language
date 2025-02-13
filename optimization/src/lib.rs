#![feature(cmp_minmax)]

mod dead_code_analysis;
mod equivalent_expression_elimination;

pub use dead_code_analysis::DeadCodeAnalyzer;
pub use equivalent_expression_elimination::EquivalentExpressionEliminator;
