//! Small pieces of JavaScript semantics the port has to keep: `String(v)`,
//! truthiness, `Math.round`, `toFixed`, `padEnd`, and the page escaper.

use serde::Serializer;
use serde_json::Value;

/// `Number.prototype.toString()` for the values this crate meets.
pub fn num_str(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if x.fract() == 0.0 && a < 1e21 {
        return format!("{}", x as i128);
    }
    if !(1e-6..1e21).contains(&a) {
        let s = format!("{x:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    format!("{x}")
}

/// `String(v)`, with `None` standing for `undefined`.
pub fn string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => num_str(n.as_f64().unwrap_or(f64::NAN)),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|e| match e {
                Value::Null => String::new(),
                other => string(Some(other)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `v.k` for any JSON value: only objects have properties here.
pub fn prop<'a>(v: Option<&'a Value>, k: &str) -> Option<&'a Value> {
    match v {
        Some(Value::Object(m)) => m.get(k),
        _ => None,
    }
}

/// `Math.round`: halves go up, also below zero.
pub fn round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// `Number.prototype.toFixed`: an exact tie picks the larger number.
pub fn to_fixed(x: f64, digits: usize) -> String {
    let p = 10f64.powi(digits as i32);
    let n = x * p;
    if (n - n.floor()) == 0.5 && x >= 0.0 {
        return format!("{:.*}", digits, (n.floor() + 1.0) / p);
    }
    format!("{x:.digits$}")
}

pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// `s.slice(0, n)` counted in UTF-16 units; a pair is never split.
pub fn slice16(s: &str, n: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        used += c.len_utf16();
        if used > n {
            break;
        }
        out.push(c);
    }
    out
}

pub fn pad_end(s: &str, n: usize) -> String {
    let len = utf16_len(s);
    if len >= n {
        return s.to_string();
    }
    format!("{s}{}", " ".repeat(n - len))
}

/// pages.js's `esc`.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// `/^\d+\.\d+\.\d+\.\d+$/` with JavaScript's ASCII `\d`.
pub fn is_dotted_quad(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Numbers that are whole serialise without a fraction, as JSON.stringify does.
pub fn ser_num<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        s.serialize_i64(*x as i64)
    } else {
        s.serialize_f64(*x)
    }
}

pub fn ser_opt_num<S: Serializer>(x: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match x {
        Some(v) => ser_num(v, s),
        None => s.serialize_none(),
    }
}

/// A whole-number f64 as JSON, the same rule as `ser_num`.
pub fn num_value(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        Value::from(x as i64)
    } else {
        serde_json::Number::from_f64(x)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_like_js() {
        assert_eq!(string(Some(&json!(1.0))), "1");
        assert_eq!(string(Some(&json!(1.5))), "1.5");
        assert_eq!(string(Some(&json!([1, null, "a"]))), "1,,a");
        assert_eq!(string(Some(&json!({"a": 1}))), "[object Object]");
        assert_eq!(string(None), "undefined");
        assert_eq!(num_str(1e21), "1e+21");
    }

    #[test]
    fn rounding_like_js() {
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(0.49999999999999994), 0.0);
        assert_eq!(to_fixed(0.25, 1), "0.3");
        assert_eq!(to_fixed(1.25, 2), "1.25");
        assert_eq!(to_fixed(14.04, 1), "14.0");
    }
}
