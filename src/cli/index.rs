use crate::index;
use std::path::PathBuf;
use std::process::ExitCode;

pub(crate) fn run(args: &[String]) -> ExitCode {
    let mut input = None;
    let mut out = None;
    let mut fields = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--field" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing field");
                    return ExitCode::from(2);
                };
                fields.push(value.clone());
                i += 2;
            }
            "-o" | "--out" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("usage: logq index <path> [-o dir] [--field name]");
                    return ExitCode::from(2);
                };
                out = Some(PathBuf::from(value));
                i += 2;
            }
            other if !other.starts_with('-') && input.is_none() => {
                input = Some(PathBuf::from(other));
                i += 1;
            }
            other => {
                eprintln!("logq: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(input) = input else {
        eprintln!("usage: logq index <path> [-o dir]");
        return ExitCode::from(2);
    };
    let out = out.unwrap_or_else(|| {
        if input.is_dir() {
            input.join(".logq")
        } else {
            PathBuf::from(".logq")
        }
    });
    match index::build(&input, &out, &fields) {
        Ok(count) => {
            println!("{count} lines -> {}", out.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("logq: {err}");
            ExitCode::from(1)
        }
    }
}
