mod parse;

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut path = None;
    let mut contains = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--contains" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("usage: logq <file> [--contains text]");
                    return ExitCode::from(2);
                };
                contains = Some(value.clone());
                i += 2;
            }
            other if !other.starts_with('-') && path.is_none() => {
                path = Some(other.to_string());
                i += 1;
            }
            other => {
                eprintln!("logq: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: logq <file> [--contains text]");
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
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("logq: {path}: {err}");
                return ExitCode::from(1);
            }
        };
        if parse::parse_event(&line, &[]).is_none() {
            continue;
        }
        if contains.as_ref().is_some_and(|text| !line.contains(text)) {
            continue;
        }
        println!("{line}");
    }
    ExitCode::SUCCESS
}
