use super::files;
use super::record::{self, REC};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    let values = files::read_json(&index_dir.join(format!("{by}.json")))?;
    let Some(list) = values.as_array() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad {by}.json"),
        ));
    };
    let toc = fs::read(index_dir.join(format!("{by}.toc")))?;
    let mut rows = Vec::new();
    for (i, value) in list.iter().enumerate() {
        let at = i * 8 + 4;
        let bytes = toc
            .get(at..at + 4)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("bad {by}.toc")))?;
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
    let meta: serde_json::Value = files::read_json(&index_dir.join("meta.json"))?;
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
    let mut chosen = None;
    if let Some(level) = &select.level {
        chosen = narrow(chosen, lookup(index_dir, "level", level)?);
    }
    if let Some(service) = &select.service {
        chosen = narrow(chosen, lookup(index_dir, "service", service)?);
    }
    for (name, value) in &select.equals {
        let key = extra_key(index_dir, name)?;
        chosen = narrow(chosen, lookup(index_dir, &key, value)?);
    }
    let ids = chosen.unwrap_or_else(|| (0..count as u32).collect());
    let mut hits = 0;
    for id in ids {
        let Some((ts, file_id, len, offset)) = record::read_record(&lines, id as usize) else {
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
        let line = record::load_line(&root, rel, offset, len)?;
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
    let meta = files::read_json(&index_dir.join("meta.json"))?;
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

fn narrow(chosen: Option<Vec<u32>>, extra: Vec<u32>) -> Option<Vec<u32>> {
    Some(match chosen {
        Some(have) => intersect(&have, &extra),
        None => extra,
    })
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
    let values = files::read_json(&index_dir.join(format!("{name}.json")))?;
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
