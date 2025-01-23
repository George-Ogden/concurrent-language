use std::io::{self, Read};

use compilation::Compiler;
use lowering::Lowerer;
use translation::Translator;
use type_checker::{Program, TypeChecker};

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    match serde_json::from_str::<Program>(&input) {
        Ok(program) => match TypeChecker::type_check(program) {
            Ok(type_checked_program) => {
                let lowered_program = Lowerer::lower(type_checked_program);
                let compiled_program = Compiler::compile(lowered_program);
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
