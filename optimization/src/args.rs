use clap::{arg, Args};

#[derive(Args)]
pub struct InliningArgs {
    #[arg(long, default_value_t = 1000)]
    pub inlining_depth: usize,
}

#[derive(Args)]
pub struct DeadCodeAnalysisArgs {
    #[arg(long)]
    pub no_dead_code_analysis: bool,
}

#[derive(Args)]
pub struct EquivalentExpressionEliminationArgs {
    #[arg(long)]
    pub no_equivalent_expression_elimination: bool,
}

#[derive(Args)]
pub struct OptimizationArgs {
    #[command(flatten)]
    pub inlining_args: InliningArgs,

    #[command(flatten)]
    pub dead_code_analysis_args: DeadCodeAnalysisArgs,

    #[command(flatten)]
    pub equivalent_elimination_args: EquivalentExpressionEliminationArgs,
}
