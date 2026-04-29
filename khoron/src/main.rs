use std::env;
use std::fs;
use std::io::{self, Read};

use khoron::Runtime;

fn main() {
    let source = match env::args().nth(1) {
        Some(path) => fs::read_to_string(&path).unwrap_or_else(|err| {
            eprintln!("failed to read {path}: {err}");
            std::process::exit(1);
        }),
        None => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .unwrap_or_else(|err| {
                    eprintln!("failed to read stdin: {err}");
                    std::process::exit(1);
                });
            source
        }
    };

    let mut runtime = Runtime::default();
    match runtime.run(&source) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
        }
        Err(err) => {
            eprintln!("khoron: {err}");
            std::process::exit(1);
        }
    }
}
