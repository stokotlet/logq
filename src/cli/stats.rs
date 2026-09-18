use crate::index;
use std::path::PathBuf;
use std::process::ExitCode;

pub(crate) fn run(args: &[String]) -> ExitCode {
    let mut out = None;
    let mut by = "level".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--by" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing field");
                    return ExitCode::from(2);
                };
                by = value.clone();
                i += 2;
            }
            "-o" | "--out" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("usage: logq stats -o <index> [--by level|service]");
                    return ExitCode::from(2);
                };
                out = Some(PathBuf::from(value));
                i += 2;
            }
            other => {
                eprintln!("logq: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(out) = out else {
        eprintln!("usage: logq stats -o <index> [--by level|service]");
        return ExitCode::from(2);
    };
    match index::counts(&out, &by) {
        Ok(rows) => {
            for (label, count) in rows {
                let name = if label.is_empty() { "(none)" } else { &label };
                println!("{name}\t{count}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("logq: {err}");
            ExitCode::from(1)
        }
    }
}
