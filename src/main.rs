mod parse;
mod scan;

use std::collections::BTreeMap;
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut path = None;
    let mut contains = None;
    let mut stats = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--stats" => {
                stats = true;
                i += 1;
            }
            "--contains" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("usage: logq <path> [--contains text] [--stats]");
                    return ExitCode::from(2);
                };
                contains = Some(value.clone());
                i += 2;
            }
            other if !other.starts_with('-') && path.is_none() => {
                path = Some(PathBuf::from(other));
                i += 1;
            }
            other => {
                eprintln!("logq: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: logq <path> [--contains text] [--stats]");
        return ExitCode::from(2);
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let result = scan::for_each_line(&path, |line| {
        let Some(event) = parse::parse_event(line, &[]) else {
            return Ok(());
        };
        if contains.as_ref().is_some_and(|text| !line.contains(text)) {
            return Ok(());
        }
        if stats {
            *counts.entry(event.level).or_default() += 1;
        } else {
            println!("{line}");
        }
        Ok(())
    });
    if stats {
        for (level, count) in &counts {
            let name = if level.is_empty() { "(none)" } else { level };
            println!("{name}\t{count}");
        }
    }
    if let Err(err) = result {
        if err.kind() != io::ErrorKind::Interrupted {
            eprintln!("logq: {}: {err}", path.display());
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
