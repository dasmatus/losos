//! The words on screen.
//!
//! Two catalogues: the web UI's own (`admin-ui/app/src/i18n`, read by
//! `build.rs` into a table at compile time), so a pane name, a row title or
//! a caption reads exactly as it does in the browser, and this client's few
//! strings of its own ([`messages`], every key in en, sk and de). Lookup and
//! plural choice follow the web UI's `lib/i18n.ts`: the form comes from the
//! `count` variable, `{name}` placeholders are filled from the variables,
//! numbers are written the way the language writes them.
//!
//! The language comes from `--lang`, else `LC_ALL`, `LC_MESSAGES` and `LANG`,
//! the way a terminal program reads it: Slovak, German, else English.

mod messages;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;

/// A plain string, or plural forms chosen by `count`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Text {
    One(&'static str),
    Plural(&'static [(&'static str, &'static str)]),
}

/// Every key carries all three languages.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Entry {
    pub(crate) en: Text,
    pub(crate) sk: Text,
    pub(crate) de: Text,
}

mod web {
    use super::{Entry, Text};
    include!(concat!(env!("OUT_DIR"), "/web_messages.rs"));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Sk,
    De,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Locale::En, Locale::Sk, Locale::De];

    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Sk => "sk",
            Locale::De => "de",
        }
    }

    fn parse(value: &str) -> Option<Locale> {
        match value.get(..2).map(str::to_ascii_lowercase).as_deref() {
            Some("en") => Some(Locale::En),
            Some("sk") => Some(Locale::Sk),
            Some("de") => Some(Locale::De),
            _ => None,
        }
    }

    /// `--lang` wins; then the POSIX locale variables, in their precedence
    /// order. The first one set decides, even when it names a language this
    /// client does not have.
    pub fn detect(flag: Option<&str>, env: impl Fn(&str) -> Option<String>) -> Locale {
        if let Some(locale) = flag.and_then(Locale::parse) {
            return locale;
        }
        for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            match env(name) {
                Some(value) if !value.is_empty() => {
                    return Locale::parse(&value).unwrap_or(Locale::En)
                }
                _ => {}
            }
        }
        Locale::En
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_locale(locale: Locale) {
    let n = match locale {
        Locale::En => 0,
        Locale::Sk => 1,
        Locale::De => 2,
    };
    CURRENT.store(n, Ordering::Relaxed);
}

thread_local! {
    static SCOPED: std::cell::Cell<Option<Locale>> = const { std::cell::Cell::new(None) };
}

/// Run `f` in `locale` on this thread only, whatever the others use: for
/// tests, which share the process.
pub fn with_locale<T>(locale: Locale, f: impl FnOnce() -> T) -> T {
    let before = SCOPED.with(|s| s.replace(Some(locale)));
    let out = f();
    SCOPED.with(|s| s.set(before));
    out
}

pub fn locale() -> Locale {
    if let Some(scoped) = SCOPED.with(|s| s.get()) {
        return scoped;
    }
    match CURRENT.load(Ordering::Relaxed) {
        1 => Locale::Sk,
        2 => Locale::De,
        _ => Locale::En,
    }
}

/// A value for a `{name}` placeholder.
#[derive(Debug, Clone)]
pub enum Arg {
    Str(String),
    Num(f64),
}

impl From<&str> for Arg {
    fn from(v: &str) -> Arg {
        Arg::Str(v.to_string())
    }
}
impl From<String> for Arg {
    fn from(v: String) -> Arg {
        Arg::Str(v)
    }
}
impl From<&String> for Arg {
    fn from(v: &String) -> Arg {
        Arg::Str(v.clone())
    }
}
macro_rules! num_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg {
            fn from(v: $t) -> Arg {
                Arg::Num(v as f64)
            }
        }
    )*};
}
num_arg!(usize, u64, u32, u16, i64, i32, f64);

struct Catalogue {
    tui: HashMap<&'static str, Entry>,
    web: HashMap<&'static str, Entry>,
}

fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| Catalogue {
        tui: messages::TUI.iter().copied().collect(),
        web: web::WEB.iter().copied().collect(),
    })
}

/// True when `key` is in either catalogue.
pub fn has_key(key: &str) -> bool {
    let c = catalogue();
    c.tui.contains_key(key) || c.web.contains_key(key)
}

/// The CLDR plural category of an integer count, for the three languages.
fn plural_category(locale: Locale, count: f64) -> &'static str {
    let whole = count.fract() == 0.0;
    match locale {
        Locale::En | Locale::De => {
            if whole && count == 1.0 {
                "one"
            } else {
                "other"
            }
        }
        Locale::Sk => {
            if !whole {
                "many"
            } else if count == 1.0 {
                "one"
            } else if (2.0..=4.0).contains(&count) {
                "few"
            } else {
                "other"
            }
        }
    }
}

fn pick(text: Text, locale: Locale, count: Option<f64>) -> &'static str {
    match text {
        Text::One(s) => s,
        Text::Plural(forms) => {
            let other = forms
                .iter()
                .find(|(f, _)| *f == "other")
                .map(|(_, s)| *s)
                .unwrap_or("");
            let Some(count) = count else { return other };
            let category = plural_category(locale, count);
            forms
                .iter()
                .find(|(f, _)| *f == category)
                .map(|(_, s)| *s)
                .unwrap_or(other)
        }
    }
}

fn entry(key: &str) -> Option<(Entry, bool)> {
    let c = catalogue();
    if let Some(e) = c.tui.get(key) {
        return Some((*e, true));
    }
    c.web.get(key).map(|e| (*e, false))
}

fn count_of(vars: &[(&str, Arg)]) -> Option<f64> {
    vars.iter().find_map(|(name, value)| match (name, value) {
        (&"count", Arg::Num(n)) => Some(*n),
        _ => None,
    })
}

/// The template for `key` in `locale`, plural form chosen, nothing filled.
/// A key in neither catalogue comes back as itself, which the tests catch.
pub fn template(key: &str, vars: &[(&str, Arg)], locale: Locale) -> String {
    match entry(key) {
        Some((e, _)) => {
            let text = match locale {
                Locale::En => e.en,
                Locale::Sk => e.sk,
                Locale::De => e.de,
            };
            pick(text, locale, count_of(vars)).to_string()
        }
        None => key.to_string(),
    }
}

fn interpolate(template: &str, vars: &[(&str, Arg)], locale: Locale) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}');
        let name = close.map(|c| &after[..c]);
        match name.filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')) {
            Some(name) => {
                match vars.iter().find(|(n, _)| *n == name) {
                    Some((_, Arg::Str(s))) => out.push_str(s),
                    Some((_, Arg::Num(n))) => out.push_str(&format_number(*n, locale)),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[name.len() + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Translate `key` into the language on screen, filling `vars`.
pub fn tv(key: &str, vars: &[(&str, Arg)]) -> String {
    let locale = locale();
    let text = interpolate(&template(key, vars, locale), vars, locale);
    match entry(key) {
        Some((_, false)) => plain_dashes(&text),
        _ => text,
    }
}

/// Translate `key` into the language on screen.
pub fn t(key: &str) -> String {
    tv(key, &[])
}

/// `tr!("key")`, or `tr!("key", name = value, ...)`.
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tv($key, &[$((stringify!($name), $crate::i18n::Arg::from($value))),+])
    };
}

/// A few of the web catalogue's sentences still carry an em dash; the
/// terminal prints them with a comma instead.
pub fn plain_dashes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '—' {
            while out.ends_with(char::is_whitespace) {
                out.pop();
            }
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            out.push_str(", ");
        } else {
            out.push(c);
        }
    }
    out
}

/// Lower-case and strip diacritics: "Úložisko" -> "ulozisko".
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect()
}

/// A number as the language writes it: grouped thousands, its own decimal
/// mark, at most three decimals.
pub fn format_number(n: f64, locale: Locale) -> String {
    format_number_with(n, locale, None, true)
}

fn separators(locale: Locale) -> (&'static str, char) {
    match locale {
        Locale::En => (",", '.'),
        Locale::Sk => ("\u{a0}", ','),
        Locale::De => (".", ','),
    }
}

/// `places`: exactly that many decimals; `None`: up to three, trailing zeros
/// dropped.
pub fn format_number_with(n: f64, locale: Locale, places: Option<usize>, group: bool) -> String {
    let (group_sep, decimal) = separators(locale);
    let fixed = match places {
        Some(p) => format!("{:.*}", p, n.abs()),
        None => {
            let s = format!("{:.3}", n.abs());
            let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
            if s.is_empty() {
                "0".to_string()
            } else {
                s
            }
        }
    };
    let (int, frac) = match fixed.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (fixed, None),
    };
    let int = if group && int.len() > 3 {
        let mut out = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push_str(group_sep);
            }
            out.push(c);
        }
        out
    } else {
        int
    };
    let sign = if n < 0.0 && fixed_is_nonzero(&int, frac.as_deref()) {
        "-"
    } else {
        ""
    };
    match frac {
        Some(f) => format!("{sign}{int}{decimal}{f}"),
        None => format!("{sign}{int}"),
    }
}

fn fixed_is_nonzero(int: &str, frac: Option<&str>) -> bool {
    int.chars().any(|c| c.is_ascii_digit() && c != '0')
        || frac.is_some_and(|f| f.chars().any(|c| c != '0'))
}

const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

/// Bytes as `df -h` counts them, the web UI's `formatBytes`.
pub fn format_bytes(bytes: Option<f64>) -> String {
    let Some(bytes) = bytes.filter(|b| b.is_finite() && *b >= 0.0) else {
        return "-".to_string();
    };
    let locale = locale();
    if bytes < 1024.0 {
        return format!("{} B", format_number(bytes.round(), locale));
    }
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let places = if value >= 10.0 { 0 } else { 1 };
    format!(
        "{} {}",
        format_number_with(value, locale, Some(places), false),
        UNITS[unit]
    )
}

/// A moment as a short local date and time, the way the web UI's
/// `toLocaleString(…, {dateStyle: "medium", timeStyle: "short"})` writes it.
pub fn format_when(at: chrono::DateTime<chrono::Utc>) -> String {
    let local = at.with_timezone(&chrono::Local);
    match locale() {
        Locale::En => local.format("%-d %b %Y, %H:%M").to_string(),
        Locale::Sk => local.format("%-d. %-m. %Y %-H:%M").to_string(),
        Locale::De => local.format("%d.%m.%Y, %H:%M").to_string(),
    }
}

/// An RFC 3339 time as [`format_when`] writes it, or the text unchanged when
/// it is not one.
pub fn format_iso(iso: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(iso) {
        Ok(at) => format_when(at.with_timezone(&chrono::Utc)),
        Err(_) => iso.to_string(),
    }
}

/// Unix seconds as [`format_when`] writes it.
pub fn format_unix(seconds: f64) -> String {
    match chrono::DateTime::from_timestamp(seconds as i64, 0) {
        Some(at) => format_when(at),
        None => "-".to_string(),
    }
}

/// Seconds ago, as words: "12 seconds", "4 minutes", "3 hours", "2 days".
pub fn format_ago(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-".to_string();
    }
    if seconds < 60.0 {
        tr!("tui.ago.seconds", count = seconds.round())
    } else if seconds < 3600.0 {
        tr!("tui.ago.minutes", count = (seconds / 60.0).round())
    } else if seconds < 86400.0 {
        tr!("tui.ago.hours", count = (seconds / 3600.0).round())
    } else {
        tr!("tui.ago.days", count = (seconds / 86400.0).round())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_has_its_three_languages_and_an_other_form() {
        for (key, e) in messages::TUI.iter().chain(web::WEB.iter()) {
            for text in [e.en, e.sk, e.de] {
                if let Text::Plural(forms) = text {
                    assert!(
                        forms.iter().any(|(f, _)| *f == "other"),
                        "{key} has no other form"
                    );
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (key, _) in messages::TUI {
            assert!(seen.insert(*key), "{key} is listed twice");
            assert!(key.starts_with("tui."), "{key} is not a tui. key");
        }
    }

    #[test]
    fn plurals_follow_each_language() {
        assert_eq!(
            template("tui.ago.seconds", &[("count", Arg::Num(1.0))], Locale::En),
            "{count} second"
        );
        assert_eq!(
            template("tui.ago.seconds", &[("count", Arg::Num(3.0))], Locale::Sk),
            "{count} sekundy"
        );
        assert_eq!(
            template("tui.ago.seconds", &[("count", Arg::Num(5.0))], Locale::Sk),
            "{count} sekúnd"
        );
        assert_eq!(
            template("tui.ago.seconds", &[("count", Arg::Num(5.0))], Locale::De),
            "{count} Sekunden"
        );
    }

    #[test]
    fn numbers_and_sizes_read_as_the_language_writes_them() {
        assert_eq!(format_number(48213.0, Locale::En), "48,213");
        assert_eq!(format_number(48213.0, Locale::De), "48.213");
        assert_eq!(format_number(48213.0, Locale::Sk), "48\u{a0}213");
        assert_eq!(format_number_with(1.5, Locale::De, Some(1), false), "1,5");
        set_locale(Locale::En);
        assert_eq!(format_bytes(Some(131.0 * 1024f64.powi(3))), "131 GB");
        assert_eq!(format_bytes(Some(1.5 * 1024f64.powi(4))), "1.5 TB");
        assert_eq!(format_bytes(None), "-");
    }

    #[test]
    fn folding_and_dashes() {
        assert_eq!(fold("Úložisko Sieť"), "ulozisko siet");
        assert_eq!(plain_dashes("one — two"), "one, two");
        assert_eq!(
            interpolate("{a} and {b} {", &[("a", "x".into())], Locale::En),
            "x and {b} {"
        );
    }

    #[test]
    fn the_locale_comes_from_the_flag_then_the_environment() {
        let env = |v: &'static str| move |name: &str| (name == "LANG").then(|| v.to_string());
        assert_eq!(Locale::detect(Some("de"), env("sk_SK.UTF-8")), Locale::De);
        assert_eq!(Locale::detect(None, env("sk_SK.UTF-8")), Locale::Sk);
        assert_eq!(Locale::detect(None, env("fr_FR.UTF-8")), Locale::En);
        assert_eq!(Locale::detect(None, |_| None), Locale::En);
    }
}
