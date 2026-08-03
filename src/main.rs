use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: logq <file>");
        return ExitCode::from(2);
    };
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("logq: {path}: {err}");
            return ExitCode::from(1);
        }
    };
    for line in BufReader::new(file).lines() {
        match line {
            Ok(line) => println!("{line}"),
            Err(err) => {
                eprintln!("logq: {path}: {err}");
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}
