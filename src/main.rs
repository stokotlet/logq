mod index;
mod parse;
mod scan;

use std::collections::BTreeMap;
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("index") {
        return index_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("query") {
        return query_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("stats") {
        return stats_cmd(&args[1..]);
    }
    let mut path = None;
    let mut contains = None;
    let mut since = None;
    let mut until = None;
    let mut stats = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--stats" => {
                stats = true;
                i += 1;
            }
            "--since" => match take_time(&args, &mut i) {
                Ok(value) => since = Some(value),
                Err(code) => return code,
            },
            "--until" => match take_time(&args, &mut i) {
                Ok(value) => until = Some(value),
                Err(code) => return code,
            },
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
        if since.is_some_and(|start| event.ts_ms < start) {
            return Ok(());
        }
        if until.is_some_and(|end| event.ts_ms >= end) {
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

fn index_cmd(args: &[String]) -> ExitCode {
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

fn query_cmd(args: &[String]) -> ExitCode {
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

fn stats_cmd(args: &[String]) -> ExitCode {
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
                    eprintln!("usage: logq stats -o <index> [--by level]");
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
        eprintln!("usage: logq stats -o <index> [--by level]");
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

fn take_time(args: &[String], i: &mut usize) -> Result<i64, ExitCode> {
    let Some(value) = args.get(*i + 1) else {
        eprintln!("logq: missing time");
        return Err(ExitCode::from(2));
    };
    *i += 2;
    parse::parse_time(value).ok_or_else(|| {
        eprintln!("logq: bad time {value}");
        ExitCode::from(2)
    })
}
