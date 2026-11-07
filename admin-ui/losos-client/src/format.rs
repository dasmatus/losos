use serde_json::Value;
pub fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
pub fn opt_i64(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64).or_else(|| {
        v.get(key)
            .and_then(Value::as_u64)
            .and_then(|n| i64::try_from(n).ok())
    })
}
pub fn opt_f64(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}
pub fn opt_bool(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}
pub fn opt_obj<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|x| x.is_object())
}
pub fn opt_array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
pub fn format_bytes(bytes: Option<f64>) -> String {
    let Some(b) = bytes else { return "–".into() };
    if b.is_nan() || b < 0.0 {
        return "–".into();
    }
    let gib = b / 1_073_741_824.0;
    if gib >= 1024.0 {
        format!("{:.1} TB", gib / 1024.0)
    } else if gib >= 10.0 {
        format!("{} GB", gib.round() as i64)
    } else if gib >= 1.0 {
        format!("{gib:.1} GB")
    } else {
        format!("{} MB", (gib * 1024.0).round() as i64)
    }
}
pub fn format_stamp(iso: Option<&str>) -> String {
    iso.map(|s| s.chars().take(16).collect::<String>().replace('T', " "))
        .unwrap_or_else(|| "–".into())
}
pub fn short_sha(sha: Option<&str>) -> String {
    sha.map(|s| s.chars().take(7).collect())
        .unwrap_or_else(|| "–".into())
}
pub fn encode_query(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}
pub fn minutes_of(hhmm: &str) -> Option<i32> {
    if hhmm.len() != 5 || hhmm.as_bytes().get(2) != Some(&b':') {
        return None;
    }
    let h: i32 = hhmm[0..2].parse().ok()?;
    let m: i32 = hhmm[3..5].parse().ok()?;
    if (0..=23).contains(&h) && (0..=59).contains(&m) {
        Some(h * 60 + m)
    } else {
        None
    }
}
pub fn describe_window(start: &str, end: &str) -> String {
    let Some(s) = minutes_of(start) else {
        return "Not set".into();
    };
    let Some(e) = minutes_of(end) else {
        return "Not set".into();
    };
    if s == e {
        return "Set, to nothing".into();
    }
    let total = if s < e { e - s } else { 1440 - s + e };
    let length = if total % 60 == 0 {
        format!("{} h", total / 60)
    } else {
        format!("{total} min")
    };
    let shape = if s > e { ", wrapping midnight" } else { "" };
    format!("{start}–{end}, {length} a day{shape}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bytes_and_window() {
        assert_eq!(format_bytes(Some(1536.0 * 1024.0 * 1024.0)), "1.5 GB");
        assert!(describe_window("22:00", "06:00").contains("wrapping midnight"));
    }
}
