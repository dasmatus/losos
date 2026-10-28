//! The admin UI's message catalogue, read from the copy its build writes
//! (`basic/messages.<locale>.json`, admin-ui/app/vite.config.ts), so a page
//! drawn here says what the script page says, in the same three languages.

use serde_json::{Map, Value};

/// The three languages the box speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Sk,
    De,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Locale::En, Locale::Sk, Locale::De];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Sk => "sk",
            Locale::De => "de",
        }
    }

    /// Each language named in itself, as the picker lists them.
    #[must_use]
    pub fn own_name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Sk => "Slovenčina",
            Locale::De => "Deutsch",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Locale> {
        match code {
            "en" => Some(Locale::En),
            "sk" => Some(Locale::Sk),
            "de" => Some(Locale::De),
            _ => None,
        }
    }

    /// The first language of an `Accept-Language` header the box has, else
    /// English: what the script page's "Browser language" picks.
    #[must_use]
    pub fn from_accept_language(header: &str) -> Locale {
        let mut tags: Vec<(f32, &str)> = header
            .split(',')
            .filter_map(|part| {
                let mut bits = part.trim().split(';');
                let tag = bits.next()?.trim();
                let q = bits
                    .find_map(|b| b.trim().strip_prefix("q="))
                    .and_then(|q| q.parse::<f32>().ok())
                    .unwrap_or(1.0);
                (!tag.is_empty()).then_some((q, tag))
            })
            .collect();
        tags.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        tags.iter()
            .find_map(|(_, tag)| {
                let base = tag.split('-').next().unwrap_or("").to_ascii_lowercase();
                Locale::from_code(&base)
            })
            .unwrap_or(Locale::En)
    }

    /// The CLDR plural category of a whole number.
    fn plural(self, n: i64) -> &'static str {
        let n = n.unsigned_abs();
        match self {
            Locale::En | Locale::De => {
                if n == 1 {
                    "one"
                } else {
                    "other"
                }
            }
            Locale::Sk => match n {
                1 => "one",
                2..=4 => "few",
                _ => "other",
            },
        }
    }

    /// A whole number the way the script page's `toLocaleString` writes it
    /// (en-GB, sk-SK, de-DE).
    #[must_use]
    pub fn number(self, n: i64) -> String {
        let sep = match self {
            Locale::En => ",",
            Locale::Sk => "\u{a0}",
            Locale::De => ".",
        };
        let digits = n.unsigned_abs().to_string();
        // sk-SK and de-DE group four-digit numbers too; en-GB as well.
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push_str(sep);
            }
            out.push(c);
        }
        if n < 0 {
            format!("-{out}")
        } else {
            out
        }
    }
}

/// A value for a `{name}` placeholder.
#[derive(Debug, Clone)]
pub enum Var {
    Text(String),
    Number(i64),
}

impl From<&str> for Var {
    fn from(s: &str) -> Self {
        Var::Text(s.to_string())
    }
}

impl From<String> for Var {
    fn from(s: String) -> Self {
        Var::Text(s)
    }
}

impl From<i64> for Var {
    fn from(n: i64) -> Self {
        Var::Number(n)
    }
}

impl From<u64> for Var {
    fn from(n: u64) -> Self {
        Var::Number(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

impl From<usize> for Var {
    fn from(n: usize) -> Self {
        Var::Number(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

/// One language's catalogue.
#[derive(Debug, Clone)]
pub struct Messages {
    pub locale: Locale,
    map: Map<String, Value>,
}

impl Messages {
    /// From the JSON the admin UI's build wrote for `locale`. A file that
    /// does not parse is an empty catalogue: every key then shows as
    /// itself, which is ugly but never a blank page.
    #[must_use]
    pub fn parse(locale: Locale, json: &[u8]) -> Messages {
        let map = match serde_json::from_slice::<Value>(json) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        Messages { locale, map }
    }

    /// A catalogue made in code, for tests.
    #[must_use]
    pub fn from_pairs(locale: Locale, pairs: &[(&str, &str)]) -> Messages {
        let map = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
            .collect();
        Messages { locale, map }
    }

    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// The text for `key`.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.tv(key, &[])
    }

    /// The text for `key` with its `{name}` placeholders filled and, for a
    /// plural entry, the form `count` asks for.
    #[must_use]
    pub fn tv(&self, key: &str, vars: &[(&str, Var)]) -> String {
        let count = vars.iter().find_map(|(k, v)| match v {
            Var::Number(n) if *k == "count" => Some(*n),
            _ => None,
        });
        let template = match self.map.get(key) {
            Some(Value::String(s)) => s.as_str(),
            Some(Value::Object(forms)) => {
                let form = count.map_or("other", |n| self.locale.plural(n));
                forms
                    .get(form)
                    .or_else(|| forms.get("other"))
                    .and_then(Value::as_str)
                    .unwrap_or(key)
            }
            _ => key,
        };
        self.fill(template, vars)
    }

    fn fill(&self, template: &str, vars: &[(&str, Var)]) -> String {
        if vars.is_empty() {
            return template.to_string();
        }
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            match after.find('}') {
                Some(close)
                    if after[..close]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
                {
                    let name = &after[..close];
                    match vars.iter().find(|(k, _)| *k == name) {
                        Some((_, Var::Text(s))) => out.push_str(s),
                        Some((_, Var::Number(n))) => out.push_str(&self.locale.number(*n)),
                        None => {
                            out.push('{');
                            out.push_str(name);
                            out.push('}');
                        }
                    }
                    rest = &after[close + 1..];
                }
                _ => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_picks_the_first_language_the_box_has() {
        assert_eq!(
            Locale::from_accept_language("fr-FR,sk;q=0.8,en;q=0.5"),
            Locale::Sk
        );
        assert_eq!(Locale::from_accept_language("de-AT"), Locale::De);
        assert_eq!(
            Locale::from_accept_language("en;q=0.1,de;q=0.9"),
            Locale::De
        );
        assert_eq!(Locale::from_accept_language(""), Locale::En);
    }

    #[test]
    fn plurals_and_placeholders() {
        let json = br#"{"x":{"one":"{count} change","few":"{count} zmeny","other":"{count} changes"},"hi":"Hi {name}"}"#;
        let en = Messages::parse(Locale::En, json);
        assert_eq!(en.tv("x", &[("count", Var::Number(1))]), "1 change");
        assert_eq!(en.tv("x", &[("count", Var::Number(1200))]), "1,200 changes");
        let sk = Messages::parse(Locale::Sk, json);
        assert_eq!(sk.tv("x", &[("count", Var::Number(3))]), "3 zmeny");
        assert_eq!(en.tv("hi", &[("name", "<Matus>".into())]), "Hi <Matus>");
        assert_eq!(en.t("missing"), "missing");
        assert_eq!(en.tv("hi", &[]), "Hi {name}");
    }
}
