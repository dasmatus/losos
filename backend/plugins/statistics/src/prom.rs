//! The Prometheus text format, as much of it as Traefik writes: one sample a
//! line, `name{label="value",...} number`, comments and blank lines skipped.

use std::collections::BTreeMap;

/// One line of the exposition.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

impl Sample {
    #[must_use]
    pub fn label(&self, name: &str) -> &str {
        self.labels.get(name).map_or("", String::as_str)
    }
}

/// Every sample in `text`. A line that does not parse is skipped rather than
/// failing the rest: a new Traefik may write something this does not know.
#[must_use]
pub fn parse(text: &str) -> Vec<Sample> {
    text.lines().filter_map(line).collect()
}

fn line(raw: &str) -> Option<Sample> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let name_end = raw.find(|c: char| c == '{' || c.is_whitespace())?;
    let name = raw[..name_end].to_string();
    let mut rest = &raw[name_end..];
    let mut labels = BTreeMap::new();
    if let Some(body) = rest.strip_prefix('{') {
        let (parsed, after) = label_set(body)?;
        labels = parsed;
        rest = after;
    }
    // A timestamp may follow the value; Traefik writes none, but allow it.
    let value = rest.split_whitespace().next()?;
    let value = match value {
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        v => v.parse().ok()?,
    };
    Some(Sample {
        name,
        labels,
        value,
    })
}

/// The labels up to the closing `}`, and what follows it.
fn label_set(mut s: &str) -> Option<(BTreeMap<String, String>, &str)> {
    let mut labels = BTreeMap::new();
    loop {
        s = s.trim_start_matches([' ', ',']);
        if let Some(after) = s.strip_prefix('}') {
            return Some((labels, after));
        }
        let eq = s.find('=')?;
        let key = s[..eq].trim().to_string();
        s = s[eq + 1..].trim_start().strip_prefix('"')?;
        let mut value = String::new();
        let mut chars = s.char_indices();
        let end = loop {
            let (i, c) = chars.next()?;
            match c {
                '\\' => match chars.next()?.1 {
                    'n' => value.push('\n'),
                    other => value.push(other),
                },
                '"' => break i,
                c => value.push(c),
            }
        };
        labels.insert(key, value);
        s = &s[end + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_traefiks_lines() {
        let text = r#"
# HELP traefik_router_requests_total How many HTTP requests are processed on a router.
# TYPE traefik_router_requests_total counter
traefik_router_requests_total{code="200",method="GET",protocol="http",router="app-notes@docker",service="app-notes@docker"} 12
traefik_router_request_duration_seconds_bucket{code="200",le="+Inf",router="app-notes@docker"} 12
traefik_open_connections{entrypoint="port-30000",protocol="TCP"} 1
process_start_time_seconds 1.76113e+09
weird{a="quote \" and \\ slash"} 3 1700000000
not a sample
"#;
        let s = parse(text);
        assert_eq!(s.len(), 5, "{s:?}");
        assert_eq!(s[0].name, "traefik_router_requests_total");
        assert_eq!(s[0].label("router"), "app-notes@docker");
        assert_eq!(s[0].label("code"), "200");
        assert_eq!(s[0].value, 12.0);
        assert_eq!(s[1].label("le"), "+Inf");
        assert_eq!(s[3].name, "process_start_time_seconds");
        assert!(s[3].labels.is_empty());
        assert_eq!(s[3].value, 1.76113e9);
        assert_eq!(s[4].label("a"), "quote \" and \\ slash");
        assert_eq!(s[4].value, 3.0);
    }
}
