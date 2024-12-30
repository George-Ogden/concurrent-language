use std::io::{self, Read};
use type_checker::Program;
use type_checker::TypeChecker;

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    dbg!(&input);
    match serde_json::from_str::<Program>(&input) {
        Ok(program) => match TypeChecker::type_check(program) {
            Ok(type_checked_program) => {
                println!("{:?}", type_checked_program)
            }
            Err(e) => {
                println!("{:?}", e)
            }
        },
        Err(msg) => println!("{}", msg),
    }
}
