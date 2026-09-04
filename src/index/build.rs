use super::dict::Dict;
use super::files::{self, Source};
use super::record::{self, REC};
use crate::parse;
use crate::scan;
use flate2::read::GzDecoder;
use serde_json::json;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub fn build(input: &Path, out: &Path, fields: &[String]) -> io::Result<usize> {
    let base = if input.is_file() {
        input
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf()
    } else {
        input.to_path_buf()
    };
    if let Some(count) = reuse(&base, input, out, fields) {
        return Ok(count);
    }
    fs::create_dir_all(out)?;
    let mut files = Vec::new();
    let mut lines = Vec::<u8>::new();
    let mut levels = Dict::new();
    let mut services = Dict::new();
    let mut extras: Vec<Dict> = fields.iter().map(|_| Dict::new()).collect();
    collect(
        &base,
        input,
        out,
        &mut files,
        &mut lines,
        &mut levels,
        &mut services,
        &mut extras,
        fields,
        input.is_file(),
    )?;
    let count = lines.len() / REC;
    fs::write(out.join("lines.dat"), &lines)?;
    levels.write(out, "level")?;
    services.write(out, "service")?;
    for (i, extra) in extras.iter().enumerate() {
        extra.write(out, &format!("extra_{i}"))?;
    }
    let meta = json!({
        "root": base.display().to_string(),
        "files": files.iter().map(|file| json!({
            "path": file.path,
            "len": file.len,
            "mtime": file.mtime,
        })).collect::<Vec<_>>(),
        "lines": count,
        "fields": fields,
    });
    fs::write(
        out.join("meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )?;
    Ok(count)
}

fn reuse(base: &Path, input: &Path, out: &Path, fields: &[String]) -> Option<usize> {
    let meta = files::read_json(&out.join("meta.json")).ok()?;
    let saved_fields: Vec<&str> = meta
        .get("fields")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let wanted: Vec<&str> = fields.iter().map(String::as_str).collect();
    if saved_fields != wanted || meta.get("root")?.as_str()? != base.display().to_string() {
        return None;
    }
    let saved = meta.get("files")?.as_array()?;
    let mut found = Vec::new();
    files::list_files(base, input, out, &mut found, input.is_file()).ok()?;
    if found.len() != saved.len() {
        return None;
    }
    for (got, prev) in found.iter().zip(saved) {
        if prev.get("path")?.as_str()? != got.path
            || prev.get("len")?.as_u64()? != got.len
            || prev.get("mtime")?.as_i64()? != got.mtime
        {
            return None;
        }
    }
    Some(meta.get("lines")?.as_u64()? as usize)
}

fn collect(
    root: &Path,
    current: &Path,
    out: &Path,
    files: &mut Vec<Source>,
    lines: &mut Vec<u8>,
    levels: &mut Dict,
    services: &mut Dict,
    extras: &mut [Dict],
    fields: &[String],
    explicit: bool,
) -> io::Result<()> {
    let meta = fs::metadata(current)?;
    if meta.is_dir() {
        if current == out {
            return Ok(());
        }
        let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) == Some(".logq") {
                continue;
            }
            collect(
                root, &path, out, files, lines, levels, services, extras, fields, false,
            )?;
        }
        return Ok(());
    }
    if !explicit && !scan::is_log(current) {
        return Ok(());
    }
    let rel = current.strip_prefix(root).unwrap_or(current).to_path_buf();
    let file_id = files.len() as u32;
    files.push(Source {
        path: rel.to_string_lossy().into_owned(),
        len: meta.len(),
        mtime: files::mtime_secs(&meta),
    });
    read_lines(current, file_id, lines, levels, services, extras, fields)?;
    Ok(())
}

fn read_lines(
    path: &Path,
    file_id: u32,
    lines: &mut Vec<u8>,
    levels: &mut Dict,
    services: &mut Dict,
    extras: &mut [Dict],
    fields: &[String],
) -> io::Result<()> {
    let file = File::open(path)?;
    let reader: Box<dyn BufRead> = if scan::is_gzip(path) {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    let mut reader = reader;
    let mut buf = Vec::new();
    let mut offset = 0u64;
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        let start = offset;
        offset += n as u64;
        let text = String::from_utf8_lossy(&buf);
        let text = text.trim_end_matches(['\r', '\n']);
        let Some(event) = parse::parse_event(text, fields) else {
            continue;
        };
        let line_id = (lines.len() / REC) as u32;
        levels.push(&event.level, line_id);
        services.push(&event.service, line_id);
        for (dict, value) in extras.iter_mut().zip(&event.extra) {
            dict.push(value, line_id);
        }
        record::push_rec(lines, event.ts_ms, file_id, start, n as u32);
    }
    Ok(())
}
