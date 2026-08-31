use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub fn for_each_line(
    path: &Path,
    mut on_line: impl FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    if meta.is_dir() {
        walk(path, &mut on_line)
    } else {
        read_file(path, &mut on_line)
    }
}

fn walk(dir: &Path, on_line: &mut impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(".logq") {
            continue;
        }
        if path.is_dir() {
            walk(&path, on_line)?;
        } else if is_log(&path) {
            read_file(&path, on_line)?;
        }
    }
    Ok(())
}

fn is_log(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with(".log")
        || name.ends_with(".jsonl")
        || name.ends_with(".log.gz")
        || name.ends_with(".jsonl.gz")
}

fn read_file(path: &Path, on_line: &mut impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
    let file = File::open(path)?;
    let reader: Box<dyn BufRead> = if is_gzip(path) {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    for line in reader.lines() {
        on_line(&line?)?;
    }
    Ok(())
}

pub fn is_gzip(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("gz")
}
