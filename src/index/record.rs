use crate::scan;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub(crate) const REC: usize = 24;

pub(crate) fn push_rec(lines: &mut Vec<u8>, ts_ms: i64, file_id: u32, offset: u64, len: u32) {
    lines.extend_from_slice(&ts_ms.to_le_bytes());
    lines.extend_from_slice(&file_id.to_le_bytes());
    lines.extend_from_slice(&len.to_le_bytes());
    lines.extend_from_slice(&offset.to_le_bytes());
}

pub(crate) fn read_record(lines: &[u8], id: usize) -> Option<(i64, u32, u32, u64)> {
    let start = id.checked_mul(REC)?;
    let rec = lines.get(start..start + REC)?;
    let ts = i64::from_le_bytes(rec[0..8].try_into().ok()?);
    let file_id = u32::from_le_bytes(rec[8..12].try_into().ok()?);
    let len = u32::from_le_bytes(rec[12..16].try_into().ok()?);
    let offset = u64::from_le_bytes(rec[16..24].try_into().ok()?);
    Some((ts, file_id, len, offset))
}

pub(crate) fn load_line(root: &Path, rel: &str, offset: u64, len: u32) -> io::Result<String> {
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
