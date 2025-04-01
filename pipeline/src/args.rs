use clap::Parser;
use optimization::OptimizationArgs;
use translation::TranslationArgs;

#[derive(Parser)]
pub struct Cli {
    #[command(flatten)]
    pub compilation_args: TranslationArgs,

    #[command(flatten)]
    pub optimization_args: OptimizationArgs,
}
