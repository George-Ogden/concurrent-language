use std::io::{self, Read};

mod args;

use args::Cli;
use clap::Parser;
use compilation::Compiler;
use emission::Emitter;
use lowering::Lowerer;
use optimization::Optimizer;
use type_checker::{Program, TypeChecker};

fn main() {
    let args = Cli::parse();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    // Deserialize the JSON from the stdin.
    match serde_json::from_str::<Program>(&input) {
        Ok(program) => match TypeChecker::type_check(program) {
            Ok(type_checked_program) => {
                let lowered_program = Lowerer::lower(type_checked_program);
                let optimized_program =
                    Optimizer::optimize(lowered_program, args.optimization_args);
                let compiled_program = Compiler::compile(optimized_program, args.compilation_args);
                let code = Emitter::emit(compiled_program);
                // Write code to the stdout.
                println!("{}", code)
            }
            Err(e) => {
                panic!("{:?}", e)
            }
        },
        Err(msg) => panic!("{}", msg),
    }
}
