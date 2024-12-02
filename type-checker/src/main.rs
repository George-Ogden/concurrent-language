mod ast_nodes;
mod type_check;
mod type_check_nodes;

use ast_nodes::*;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");
    let node = serde_json::from_str::<TypeInstance>(&input);
    println!("{:?}", node);
}
