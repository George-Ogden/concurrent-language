use lowering::IntermediateProgram;

use crate::{
    args::OptimizationArgs, dead_code_analysis::DeadCodeAnalyzer,
    equivalent_expression_elimination::EquivalentExpressionEliminator, inlining::Inliner,
};

pub struct Optimizer {}

impl Optimizer {
    pub fn optimize(
        mut program: IntermediateProgram,
        args: OptimizationArgs,
    ) -> IntermediateProgram {
        if !args.dead_code_analysis_args.no_dead_code_analysis {
            program = DeadCodeAnalyzer::remove_dead_code(program);
        }
        if !args
            .equivalent_elimination_args
            .no_equivalent_expression_elimination
        {
            program = EquivalentExpressionEliminator::eliminate_equivalent_expressions(program);
        }
        program = Inliner::inline_up_to_size(program, Some(args.inlining_args.inlining_depth));
        program
    }
}
