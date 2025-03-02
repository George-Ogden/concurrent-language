use clap::{arg, Args};

#[derive(Args)]
pub struct CompilationArgs {
    #[arg(long)]
    pub export_vector_file: Option<String>,
}
