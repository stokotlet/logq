use chrono::DateTime;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub ts_ms: i64,
    pub level: String,
    pub service: String,
    pub msg: String,
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
        msg: field(obj, &["msg", "message"]),
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
    if number.abs() >= 10_000_000_000 {
        number
    } else {
        number.saturating_mul(1000)
    }
}
