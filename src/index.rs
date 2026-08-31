use crate::parse;
use crate::scan;
use flate2::read::GzDecoder;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

struct Dict {
    to_id: HashMap<String, u32>,
    values: Vec<String>,
    ids: Vec<u32>,
    posts: Vec<Vec<u32>>,
}

impl Dict {
    fn new() -> Self {
        Self {
            to_id: HashMap::new(),
            values: Vec::new(),
            ids: Vec::new(),
            posts: Vec::new(),
        }
    }

    fn push(&mut self, value: &str, line: u32) {
        let id = if let Some(id) = self.to_id.get(value) {
            *id
        } else {
            let id = self.values.len() as u32;
            self.to_id.insert(value.to_string(), id);
            self.values.push(value.to_string());
            self.posts.push(Vec::new());
            id
        };
        self.ids.push(id);
        self.posts[id as usize].push(line);
    }

    fn write(&self, out: &Path, name: &str) -> io::Result<()> {
        let listed = serde_json::to_vec_pretty(&self.values).map_err(io::Error::other)?;
        fs::write(out.join(format!("{name}.json")), listed)?;
        let mut col = Vec::with_capacity(self.ids.len() * 4);
        for id in &self.ids {
            col.extend_from_slice(&id.to_le_bytes());
        }
        fs::write(out.join(format!("{name}.col")), col)?;
        let mut toc = Vec::new();
        let mut posted = Vec::new();
        let mut start = 0u32;
        for bucket in &self.posts {
            toc.extend_from_slice(&start.to_le_bytes());
            toc.extend_from_slice(&(bucket.len() as u32).to_le_bytes());
            for line in bucket {
                posted.extend_from_slice(&line.to_le_bytes());
            }
            start += bucket.len() as u32;
        }
        fs::write(out.join(format!("{name}.toc")), toc)?;
        fs::write(out.join(format!("{name}.ids")), posted)?;
        Ok(())
    }
}

const REC: usize = 24;

#[derive(Clone)]
struct Source {
    path: String,
    len: u64,
    mtime: i64,
}

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
    let meta = read_json(&out.join("meta.json")).ok()?;
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
    list_files(base, input, out, &mut found, input.is_file()).ok()?;
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

fn list_files(
    root: &Path,
    current: &Path,
    out: &Path,
    files: &mut Vec<Source>,
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
            list_files(root, &path, out, files, false)?;
        }
        return Ok(());
    }
    if !explicit && !is_log(current) {
        return Ok(());
    }
    let rel = current.strip_prefix(root).unwrap_or(current);
    files.push(Source {
        path: rel.to_string_lossy().into_owned(),
        len: meta.len(),
        mtime: mtime_secs(&meta),
    });
    Ok(())
}

fn mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
    if !explicit && !is_log(current) {
        return Ok(());
    }
    let rel = current.strip_prefix(root).unwrap_or(current).to_path_buf();
    let file_id = files.len() as u32;
    files.push(Source {
        path: rel.to_string_lossy().into_owned(),
        len: meta.len(),
        mtime: mtime_secs(&meta),
    });
    read_lines(current, file_id, lines, levels, services, extras, fields)?;
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
        push_rec(lines, event.ts_ms, file_id, start, n as u32);
    }
    Ok(())
}

fn push_rec(lines: &mut Vec<u8>, ts_ms: i64, file_id: u32, offset: u64, len: u32) {
    lines.extend_from_slice(&ts_ms.to_le_bytes());
    lines.extend_from_slice(&file_id.to_le_bytes());
    lines.extend_from_slice(&len.to_le_bytes());
    lines.extend_from_slice(&offset.to_le_bytes());
}

#[derive(Default)]
pub struct Select {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub level: Option<String>,
    pub service: Option<String>,
    pub contains: Option<String>,
    pub equals: Vec<(String, String)>,
}

pub fn counts(index_dir: &Path, by: &str) -> io::Result<Vec<(String, usize)>> {
    if by != "level" && by != "service" {
        return Err(io::Error::other(format!("can't count by {by}")));
    }
    let values = read_json(&index_dir.join(format!("{by}.json")))?;
    let Some(list) = values.as_array() else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad {by}.json")));
    };
    let toc = fs::read(index_dir.join(format!("{by}.toc")))?;
    let mut rows = Vec::new();
    for (i, value) in list.iter().enumerate() {
        let at = i * 8 + 4;
        let bytes = toc.get(at..at + 4).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, format!("bad {by}.toc"))
        })?;
        let count = u32::from_le_bytes(bytes.try_into().unwrap()) as usize;
        if count == 0 {
            continue;
        }
        let label = value.as_str().unwrap_or("").to_string();
        rows.push((label, count));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(rows)
}

pub fn each_line(
    index_dir: &Path,
    select: &Select,
    mut on_line: impl FnMut(&str) -> io::Result<()>,
) -> io::Result<usize> {
    let meta: serde_json::Value = read_json(&index_dir.join("meta.json"))?;
    let root = PathBuf::from(meta["root"].as_str().unwrap_or("."));
    let files = meta["files"]
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "meta.json has no files"))?;
    let paths: Vec<String> = files
        .iter()
        .map(|file| file["path"].as_str().unwrap_or("").to_string())
        .collect();
    let lines = fs::read(index_dir.join("lines.dat"))?;
    let count = lines.len() / REC;
    let mut chosen = if let Some(level) = &select.level {
        Some(lookup(index_dir, "level", level)?)
    } else {
        None
    };
    if let Some(service) = &select.service {
        let extra = lookup(index_dir, "service", service)?;
        chosen = Some(match chosen {
            Some(have) => intersect(&have, &extra),
            None => extra,
        });
    }
    for (name, value) in &select.equals {
        let key = extra_key(index_dir, name)?;
        let extra = lookup(index_dir, &key, value)?;
        chosen = Some(match chosen {
            Some(have) => intersect(&have, &extra),
            None => extra,
        });
    }
    let ids = chosen.unwrap_or_else(|| (0..count as u32).collect());
    let mut hits = 0;
    for id in ids {
        let Some((ts, file_id, len, offset)) = read_record(&lines, id as usize) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated lines.dat",
            ));
        };
        if select.since.is_some_and(|start| ts < start) {
            continue;
        }
        if select.until.is_some_and(|end| ts >= end) {
            continue;
        }
        let rel = paths
            .get(file_id as usize)
            .map(String::as_str)
            .unwrap_or("");
        let line = load_line(&root, rel, offset, len)?;
        if select
            .contains
            .as_ref()
            .is_some_and(|text| !line.contains(text))
        {
            continue;
        }
        on_line(&line)?;
        hits += 1;
    }
    Ok(hits)
}

fn extra_key(index_dir: &Path, name: &str) -> io::Result<String> {
    let meta = read_json(&index_dir.join("meta.json"))?;
    let Some(fields) = meta["fields"].as_array() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "index has no fields",
        ));
    };
    let Some(pos) = fields.iter().position(|field| field.as_str() == Some(name)) else {
        return Err(io::Error::other(format!("field {name} was not indexed")));
    };
    Ok(format!("extra_{pos}"))
}

fn intersect(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(left[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

fn lookup(index_dir: &Path, name: &str, value: &str) -> io::Result<Vec<u32>> {
    let values = read_json(&index_dir.join(format!("{name}.json")))?;
    let Some(list) = values.as_array() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad {name}.json"),
        ));
    };
    let Some(pos) = list.iter().position(|item| item.as_str() == Some(value)) else {
        return Ok(Vec::new());
    };
    let toc = fs::read(index_dir.join(format!("{name}.toc")))?;
    let ids = fs::read(index_dir.join(format!("{name}.ids")))?;
    let at = pos * 8;
    let toc_bytes = toc
        .get(at..at + 8)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("bad {name}.toc")))?;
    let start = u32::from_le_bytes(toc_bytes[0..4].try_into().unwrap());
    let count = u32::from_le_bytes(toc_bytes[4..8].try_into().unwrap());
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let off = (start as usize + i as usize) * 4;
        let bytes = ids
            .get(off..off + 4)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("bad {name}.ids")))?;
        out.push(u32::from_le_bytes(bytes.try_into().unwrap()));
    }
    Ok(out)
}

fn read_json(path: &Path) -> io::Result<serde_json::Value> {
    let bytes = fs::read(path)
        .map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {err}", path.display()),
        )
    })
}

pub fn read_record(lines: &[u8], id: usize) -> Option<(i64, u32, u32, u64)> {
    let start = id.checked_mul(REC)?;
    let rec = lines.get(start..start + REC)?;
    let ts = i64::from_le_bytes(rec[0..8].try_into().ok()?);
    let file_id = u32::from_le_bytes(rec[8..12].try_into().ok()?);
    let len = u32::from_le_bytes(rec[12..16].try_into().ok()?);
    let offset = u64::from_le_bytes(rec[16..24].try_into().ok()?);
    Some((ts, file_id, len, offset))
}

pub fn load_line(root: &Path, rel: &str, offset: u64, len: u32) -> io::Result<String> {
    let path = root.join(rel);
    let file = File::open(&path)?;
    if scan::is_gzip(&path) {
        let mut decoder = GzDecoder::new(file);
        skip(&mut decoder, offset)?;
        let mut buf = vec![0u8; len as usize];
        decoder.read_exact(&mut buf)?;
        return Ok(trim_line(&buf));
    }
    let mut file = file;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len as usize];
    file.read_exact(&mut buf)?;
    Ok(trim_line(&buf))
}

fn skip(reader: &mut impl Read, mut left: u64) -> io::Result<()> {
    let mut buf = [0u8; 8192];
    while left > 0 {
        let n = std::cmp::min(left, buf.len() as u64) as usize;
        reader.read_exact(&mut buf[..n])?;
        left -= n as u64;
    }
    Ok(())
}

fn trim_line(buf: &[u8]) -> String {
    String::from_utf8_lossy(buf)
        .trim_end_matches(['\r', '\n'])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_line_table() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("logq-lines-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("app.log"),
            "{\"ts\":\"2026-08-07T12:00:00Z\",\"level\":\"error\",\"msg\":\"nope\"}\nnot json\n",
        )
        .unwrap();
        let out = dir.join("idx");
        let count = build(&dir, &out, &[]).unwrap();
        assert_eq!(count, 1);
        let levels: Vec<String> =
            serde_json::from_slice(&fs::read(out.join("level.json")).unwrap()).unwrap();
        assert_eq!(levels, vec!["error".to_string()]);
        let bytes = fs::read(out.join("lines.dat")).unwrap();
        let (ts, file_id, len, offset) = read_record(&bytes, 0).unwrap();
        assert_eq!(ts, 1_786_104_000_000);
        assert_eq!(file_id, 0);
        let line = load_line(&dir, "app.log", offset, len).unwrap();
        assert!(line.contains("nope"), "{line}");
        let mut printed = String::new();
        each_line(&out, &Select::default(), |line| {
            printed = line.to_string();
            Ok(())
        })
        .unwrap();
        assert!(printed.contains("nope"), "{printed}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn indexes_a_single_file() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("logq-one-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        let log = dir.join("app.log");
        fs::write(
            &log,
            "{\"ts\":\"2026-08-07T12:00:00Z\",\"level\":\"info\",\"msg\":\"ok\"}\n",
        )
        .unwrap();
        let out = dir.join("idx");
        assert_eq!(build(&log, &out, &[]).unwrap(), 1);
        let mut printed = String::new();
        each_line(&out, &Select::default(), |line| {
            printed = line.to_string();
            Ok(())
        })
        .unwrap();
        assert!(printed.contains("ok"), "{printed}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn filters_by_field_and_text() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("logq-filter-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("app.log"),
            concat!(
                "{\"ts\":\"2026-08-07T12:00:00Z\",\"level\":\"error\",\"service\":\"checkout\",\"status\":500,\"msg\":\"declined\"}\n",
                "{\"ts\":\"2026-08-07T12:05:00Z\",\"level\":\"error\",\"service\":\"checkout\",\"status\":500,\"msg\":\"timeout\"}\n",
                "{\"ts\":\"2026-08-07T12:06:00Z\",\"level\":\"info\",\"service\":\"api\",\"status\":200,\"msg\":\"ok\"}\n",
            ),
        )
        .unwrap();
        let out = dir.join("idx");
        let fields = vec!["status".to_string()];
        build(&dir, &out, &fields).unwrap();
        let select = Select {
            level: Some("error".to_string()),
            service: Some("checkout".to_string()),
            contains: Some("declined".to_string()),
            equals: vec![("status".to_string(), "500".to_string())],
            ..Select::default()
        };
        let mut hits = Vec::new();
        each_line(&out, &select, |line| {
            hits.push(line.to_string());
            Ok(())
        })
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].contains("declined"));
        let before = fs::metadata(out.join("lines.dat"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        build(&dir, &out, &fields).unwrap();
        let after = fs::metadata(out.join("lines.dat"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(dir);
    }
}
