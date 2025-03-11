use clap::Parser;
use compilation::CompilationArgs;
use optimization::OptimizationArgs;

#[derive(Parser)]
pub struct Cli {
    #[command(flatten)]
    pub compilation_args: CompilationArgs,

    #[command(flatten)]
    pub optimization_args: OptimizationArgs,
}
