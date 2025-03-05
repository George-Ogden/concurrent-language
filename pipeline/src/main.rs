use std::io::{self, Read};

mod args;

use args::Cli;
use clap::Parser;
use compilation::Compiler;
use lowering::Lowerer;
use optimization::{DeadCodeAnalyzer, EquivalentExpressionEliminator, Inliner};
use translation::Translator;
use type_checker::{Program, TypeChecker};

fn main() {
    let args = Cli::parse();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    match serde_json::from_str::<Program>(&input) {
        Ok(program) => match TypeChecker::type_check(program) {
            Ok(type_checked_program) => {
                let mut lowered_program = Lowerer::lower(type_checked_program);
                for optimization in [
                    DeadCodeAnalyzer::remove_dead_code,
                    EquivalentExpressionEliminator::eliminate_equivalent_expressions,
                    |program| Inliner::inline_up_to_size(program, Some(1000)),
                ] {
                    lowered_program = optimization(lowered_program);
                }
                let compiled_program = Compiler::compile(lowered_program, args.compilation_args);
                let code = Translator::translate(compiled_program);
                println!("{}", code)
            }
            Err(e) => {
                panic!("{:?}", e)
            }
        },
        Err(msg) => panic!("{}", msg),
    }
}
