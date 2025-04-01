use clap::{arg, Args};

#[derive(Args)]
pub struct TranslationArgs {
    #[arg(long)]
    pub export_vector_file: Option<String>,
}
