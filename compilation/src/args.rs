use clap::{arg, Args};

#[derive(Args)]
pub struct CompilationArgs {
    #[arg(long)]
    export_vector_file: Option<String>,
}
