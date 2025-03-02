use clap::Parser;
use compilation::CompilationArgs;

#[derive(Parser)]
pub struct Cli {
    #[command(flatten)]
    pub compilation_args: CompilationArgs,
}
