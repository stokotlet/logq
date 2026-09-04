use super::build::build;
use super::query::{Select, each_line};
use super::record::{load_line, read_record};
use std::fs;
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
