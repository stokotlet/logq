use std::fs::{self, Metadata};
use std::io;
use std::path::Path;

pub(crate) struct Source {
    pub(crate) path: String,
    pub(crate) len: u64,
    pub(crate) mtime: i64,
}

pub(crate) fn list_files(
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
    if !explicit && !crate::scan::is_log(current) {
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

pub(crate) fn mtime_secs(meta: &Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn read_json(path: &Path) -> io::Result<serde_json::Value> {
    let bytes = fs::read(path)
        .map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {err}", path.display()),
        )
    })
}
