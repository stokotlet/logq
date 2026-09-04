use super::take_time;
use crate::index;
use std::path::PathBuf;
use std::process::ExitCode;

pub(crate) fn run(args: &[String]) -> ExitCode {
    let mut out = None;
    let mut select = index::Select::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--contains" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing text");
                    return ExitCode::from(2);
                };
                select.contains = Some(value.clone());
                i += 2;
            }
            "--field" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing field");
                    return ExitCode::from(2);
                };
                let Some((name, wanted)) = value.split_once('=') else {
                    eprintln!("logq: field must look like name=value");
                    return ExitCode::from(2);
                };
                select.equals.push((name.to_string(), wanted.to_string()));
                i += 2;
            }
            "--service" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing service");
                    return ExitCode::from(2);
                };
                select.service = Some(value.clone());
                i += 2;
            }
            "--level" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("logq: missing level");
                    return ExitCode::from(2);
                };
                select.level = Some(value.clone());
                i += 2;
            }
            "--since" => match take_time(args, &mut i) {
                Ok(value) => select.since = Some(value),
                Err(code) => return code,
            },
            "--until" => match take_time(args, &mut i) {
                Ok(value) => select.until = Some(value),
                Err(code) => return code,
            },
            "-o" | "--out" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("usage: logq query -o <index> [--since time] [--until time]");
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
        eprintln!("usage: logq query -o <index> [--since time] [--until time]");
        return ExitCode::from(2);
    };
    match index::each_line(&out, &select, |line| {
        println!("{line}");
        Ok(())
    }) {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("logq: {err}");
            ExitCode::from(1)
        }
    }
}
