mod translation;

use lowering::Program;
use std::io::{self, Read};
use translation::Translator;

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    dbg!(&input);
    match serde_json::from_str::<Program>(&input) {
        Ok(program) => {
            let code = Translator::translate(program);
            println!("{}", code)
        }
        Err(msg) => println!("{}", msg),
    }
}
