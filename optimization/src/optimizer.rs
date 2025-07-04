use lowering::IntermediateProgram;

use crate::{
    args::OptimizationArgs, dead_code_analysis::DeadCodeAnalyzer, inlining::Inliner,
    redundancy_elimination::RedundancyEliminator,
};

pub struct Optimizer {}

impl Optimizer {
    pub fn optimize(
        mut program: IntermediateProgram,
        args: OptimizationArgs,
    ) -> IntermediateProgram {
        if !args.dead_code_analysis_args.no_dead_code_analysis {
            program = DeadCodeAnalyzer::remove_dead_code(program);
            program = DeadCodeAnalyzer::remove_dead_code(program);
        }
        if !args
            .equivalent_elimination_args
            .no_equivalent_expression_elimination
        {
            program = RedundancyEliminator::eliminate_redundancy(program);
        }
        program = Inliner::inline_up_to_size(program, Some(args.inlining_args.inlining_depth));
        program
    }
}
