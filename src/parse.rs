use chrono::DateTime;
use serde_json::{Map, Value};

#[derive(Debug)]
pub struct Event {
    pub ts_ms: i64,
    pub level: String,
    pub service: String,
    pub extra: Vec<String>,
}

pub fn parse_event(line: &str, extra_keys: &[String]) -> Option<Event> {
    let value: Value = serde_json::from_str(line.trim_end_matches('\r')).ok()?;
    let obj = value.as_object()?;
    let ts_ms = timestamp_ms(obj)?;
    Some(Event {
        ts_ms,
        level: field(obj, &["level", "lvl", "severity"]),
        service: field(obj, &["service", "svc", "logger"]),
        extra: extra_keys
            .iter()
            .map(|key| field(obj, &[key.as_str()]))
            .collect(),
    })
}

fn field(obj: &Map<String, Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = obj.get(*key) {
            return as_string(value);
        }
    }
    String::new()
}

fn as_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn timestamp_ms(obj: &Map<String, Value>) -> Option<i64> {
    let raw = obj
        .get("ts")
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("timestamp"))
        .or_else(|| obj.get("@timestamp"))?;
    match raw {
        Value::Number(number) => number.as_i64().map(normalize_epoch),
        Value::String(text) => parse_time(text),
        _ => None,
    }
}

pub fn parse_time(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(number) = text.parse::<i64>() {
        return Some(normalize_epoch(number));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp_millis());
    }
    let with_t = text.replace(' ', "T");
    DateTime::parse_from_rfc3339(&with_t)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn normalize_epoch(number: i64) -> i64 {
    // 10 digits is still seconds. Past that, treat it as milliseconds.
    if number.abs() >= 10_000_000_000 {
        number
    } else {
        number.saturating_mul(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_common_keys() {
        let line = r#"{"ts":"2026-08-07T12:00:00Z","lvl":"error","logger":"checkout","message":"declined"}"#;
        let event = parse_event(line, &[]).unwrap();
        assert_eq!(event.level, "error");
        assert_eq!(event.service, "checkout");
        assert_eq!(event.ts_ms, 1_786_104_000_000);
    }

    #[test]
    fn parses_offset_and_space() {
        let zulu = parse_time("2026-08-07T12:00:00Z").unwrap();
        let plus = parse_time("2026-08-07T17:00:00+05:00").unwrap();
        assert_eq!(zulu, plus);
        let spaced = parse_time("2026-08-07 12:00:00Z").unwrap();
        assert_eq!(zulu, spaced);
        assert_eq!(parse_time("1786104000").unwrap(), zulu);
        assert_eq!(parse_time("1786104000000").unwrap(), zulu);
    }

    #[test]
    fn skips_plain_text() {
        assert!(parse_event("not json", &[]).is_none());
        assert!(parse_event(r#"{"msg":"no time"}"#, &[]).is_none());
    }
}
