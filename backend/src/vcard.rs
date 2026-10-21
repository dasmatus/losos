//! vCards in and out of LosOS Contacts (`crate::cloud::contacts`).
//!
//! [`parse`] reads what phones and address books write: vCard 4.0
//! (RFC 6350), 3.0 (RFC 2426) and the 2.1 that older Android and Outlook
//! exports still carry, one card or many in one file. It is lenient on
//! purpose, because every exporter bends the format somewhere: lines are
//! unfolded, group prefixes (`item1.TEL`) are dropped but their Apple
//! `X-ABLabel` is kept as the label, 2.1's bare parameters (`TEL;CELL`) and
//! quoted-printable values are understood, and a line it cannot read is
//! skipped rather than failing the file. What comes out is
//! [`Contact`]s, the fields the Contacts page edits.
//!
//! [`write`] writes 4.0 or 3.0. 3.0 is what the export offers first: every
//! phone imports it, while 4.0's `data:` photos and `tel:` values still
//! trip some. [`rewrite`] writes one card over one a phone or desktop app
//! stored, keeping the lines this module does not read.
//!
//! Every value is bounded ([`MAX_FIELD`], [`MAX_LIST`]), so a hostile file
//! cannot grow one contact without end.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// The longest text a field keeps, in characters.
pub const MAX_FIELD: usize = 500;
/// The longest note a contact keeps, in characters.
pub const MAX_NOTE: usize = 4000;
/// The most emails, phones, addresses or links one contact keeps.
pub const MAX_LIST: usize = 16;
/// The largest photo, in bytes before base64, a card may carry into the
/// importer (it is scaled down afterwards).
pub const MAX_PHOTO_IN: usize = 4 * 1024 * 1024;

/// A name split into its parts (`N`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Name {
    pub family: String,
    pub given: String,
    pub additional: String,
    pub prefix: String,
    pub suffix: String,
}

impl Name {
    pub fn is_empty(&self) -> bool {
        [
            &self.family,
            &self.given,
            &self.additional,
            &self.prefix,
            &self.suffix,
        ]
        .iter()
        .all(|s| s.trim().is_empty())
    }

    /// "Prefix Given Additional Family Suffix", skipping empty parts.
    pub fn joined(&self) -> String {
        [
            &self.prefix,
            &self.given,
            &self.additional,
            &self.family,
            &self.suffix,
        ]
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
    }
}

/// An email, phone or link with what kind it is: `home`, `work`,
/// `mobile`, `fax`, `other`, or a label the exporter wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Labelled {
    pub value: String,
    pub label: String,
}

/// A postal address (`ADR`), without the post office box and extended
/// parts, which nobody fills in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Address {
    pub street: String,
    pub locality: String,
    pub region: String,
    pub code: String,
    pub country: String,
    pub label: String,
}

impl Address {
    pub fn is_empty(&self) -> bool {
        [
            &self.street,
            &self.locality,
            &self.region,
            &self.code,
            &self.country,
        ]
        .iter()
        .all(|s| s.trim().is_empty())
    }
}

/// One contact, as a card gave it and as the Contacts page edits it. The
/// photo is not here: it travels apart ([`Card::photo`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Contact {
    /// What the contact is called (`FN`).
    pub name: String,
    #[serde(skip_serializing_if = "Name::is_empty")]
    pub n: Name,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nickname: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub org: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<Labelled>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<Labelled>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<Address>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<Labelled>,
    /// `YYYY-MM-DD`, or `--MM-DD` when the year is not known.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub birthday: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// A Matrix user ID, `@name:server`, to chat with or call.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub matrix: String,
    /// The person's mesh address, `name@box`, to share files with.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mesh: String,
    /// The card's `UID`, so importing the same file again updates rather
    /// than doubles.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub uid: String,
}

impl Contact {
    /// Clean every field and fill `name` from what the card has, the way a
    /// form's save does too. `false` when nothing is left to call it by.
    pub fn tidy(&mut self) -> bool {
        let field = |s: &mut String| *s = clip(s.trim(), MAX_FIELD);
        field(&mut self.name);
        field(&mut self.n.family);
        field(&mut self.n.given);
        field(&mut self.n.additional);
        field(&mut self.n.prefix);
        field(&mut self.n.suffix);
        field(&mut self.nickname);
        field(&mut self.org);
        field(&mut self.title);
        field(&mut self.uid);
        self.note = clip(self.note.trim(), MAX_NOTE);
        let list = |l: &mut Vec<Labelled>, keep: fn(&str) -> Option<String>| {
            let mut seen = std::collections::HashSet::new();
            l.retain_mut(|x| {
                x.label = label_word(&x.label);
                match keep(x.value.trim()) {
                    Some(v) if seen.insert(v.to_lowercase()) => {
                        x.value = v;
                        true
                    }
                    _ => false,
                }
            });
            l.truncate(MAX_LIST);
        };
        list(&mut self.emails, |v| {
            let v = v.strip_prefix("mailto:").unwrap_or(v).trim();
            (v.contains('@') && !v.contains(char::is_whitespace) && v.len() <= 254)
                .then(|| v.to_string())
        });
        list(&mut self.phones, |v| {
            let v = v.strip_prefix("tel:").unwrap_or(v).trim();
            (!v.is_empty() && v.chars().any(|c| c.is_ascii_digit())).then(|| clip(v, 64))
        });
        list(&mut self.urls, |v| {
            (v.starts_with("https://") || v.starts_with("http://"))
                .then(|| clip(v, MAX_FIELD))
                .filter(|v| !v.contains(char::is_whitespace))
        });
        for a in &mut self.addresses {
            field(&mut a.street);
            field(&mut a.locality);
            field(&mut a.region);
            field(&mut a.code);
            field(&mut a.country);
            a.label = label_word(&a.label);
        }
        self.addresses.retain(|a| !a.is_empty());
        self.addresses.truncate(MAX_LIST);
        self.birthday = birthday(&self.birthday).unwrap_or_default();
        self.matrix = matrix_id(&self.matrix).unwrap_or_default();
        self.mesh = mesh_address(&self.mesh).unwrap_or_default();
        if self.name.is_empty() {
            self.name = clip(&self.n.joined(), MAX_FIELD);
        }
        for fallback in [
            self.nickname.clone(),
            self.org.clone(),
            self.emails
                .first()
                .map(|e| e.value.clone())
                .unwrap_or_default(),
            self.phones
                .first()
                .map(|p| p.value.clone())
                .unwrap_or_default(),
        ] {
            if self.name.is_empty() {
                self.name = fallback;
            }
        }
        !self.name.is_empty()
    }
}

/// One card from a file: the contact and the photo it carried, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Card {
    pub contact: Contact,
    pub photo: Option<Vec<u8>>,
}

/// What [`parse`] made of a file.
#[derive(Debug, Default)]
pub struct Parsed {
    pub cards: Vec<Card>,
    /// Cards with nothing to call the contact by.
    pub skipped: usize,
}

/// The cards in a file, in order.
pub fn parse(bytes: &[u8]) -> Parsed {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let mut out = Parsed::default();
    let mut lines: Vec<Line> = Vec::new();
    let mut depth = 0usize;
    for raw in unfold(&text) {
        let Some(line) = Line::read(&raw) else {
            continue;
        };
        match (
            line.name.as_str(),
            line.raw.trim().to_ascii_uppercase().as_str(),
        ) {
            ("BEGIN", "VCARD") => {
                depth += 1;
                if depth == 1 {
                    lines.clear();
                }
                continue;
            }
            ("END", "VCARD") => {
                if depth == 1 {
                    match card(&lines) {
                        Some(c) => out.cards.push(c),
                        None => out.skipped += 1,
                    }
                }
                depth = depth.saturating_sub(1);
                continue;
            }
            _ => {}
        }
        // A card nested inside another (2.1's AGENT) is not a contact of
        // its own and not part of the outer one.
        if depth == 1 {
            lines.push(line);
        }
    }
    out
}

/// One content line: `group.NAME;PARAM=a,b;BARE:value`.
#[derive(Debug, Clone)]
struct Line {
    group: String,
    name: String,
    /// Parameters, names upper case, values as written.
    params: Vec<(String, String)>,
    /// The value, still escaped, already quoted-printable decoded.
    raw: String,
}

impl Line {
    fn read(line: &str) -> Option<Line> {
        // The colon that ends the name is the first one outside quotes.
        let mut quoted = false;
        let mut at = None;
        for (i, c) in line.char_indices() {
            match c {
                '"' => quoted = !quoted,
                ':' if !quoted => {
                    at = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let at = at?;
        let (head, value) = (&line[..at], &line[at + 1..]);
        let mut parts = split_unquoted(head, ';').into_iter();
        let full = parts.next()?.trim().to_string();
        let (group, name) = match full.rsplit_once('.') {
            Some((g, n)) => (g.to_string(), n.to_ascii_uppercase()),
            None => (String::new(), full.to_ascii_uppercase()),
        };
        if name.is_empty() {
            return None;
        }
        let params: Vec<(String, String)> = parts
            .map(|p| match p.split_once('=') {
                Some((k, v)) => (
                    k.trim().to_ascii_uppercase(),
                    v.trim().trim_matches('"').to_string(),
                ),
                // 2.1: a bare parameter is a TYPE, or an ENCODING.
                None => {
                    let p = p.trim().to_ascii_uppercase();
                    if p == "QUOTED-PRINTABLE" || p == "BASE64" {
                        ("ENCODING".to_string(), p)
                    } else {
                        ("TYPE".to_string(), p)
                    }
                }
            })
            .collect();
        let mut line = Line {
            group,
            name,
            params,
            raw: value.to_string(),
        };
        if line
            .param("ENCODING")
            .is_some_and(|e| e.eq_ignore_ascii_case("QUOTED-PRINTABLE"))
        {
            let bytes = quoted_printable(&line.raw);
            line.raw = match line.param("CHARSET") {
                Some(c) if !c.eq_ignore_ascii_case("UTF-8") => latin1(&bytes),
                _ => String::from_utf8_lossy(&bytes).into_owned(),
            };
        }
        Some(line)
    }

    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Every TYPE value, lower case, `TYPE=a,b` and repeated `TYPE`s alike.
    fn types(&self) -> Vec<String> {
        self.params
            .iter()
            .filter(|(k, _)| k == "TYPE")
            .flat_map(|(_, v)| v.split(','))
            .map(|t| t.trim().trim_matches('"').to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect()
    }

    fn text(&self) -> String {
        unescape(&self.raw)
    }

    fn parts(&self) -> Vec<String> {
        split_escaped(&self.raw, ';')
            .iter()
            .map(|p| unescape(p))
            .collect()
    }
}

/// Lines with folding undone: a line that starts with a space or a tab
/// continues the one before, and a quoted-printable line that ends in `=`
/// continues on the next.
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut soft = false;
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        match out.last_mut() {
            Some(last) if soft => last.push_str(line),
            Some(last) if line.starts_with(' ') || line.starts_with('\t') => {
                last.push_str(&line[1..])
            }
            _ if line.trim().is_empty() => {
                soft = false;
                continue;
            }
            _ => out.push(line.to_string()),
        }
        soft = false;
        if let Some(last) = out.last_mut() {
            if last.ends_with('=') && qp_line(last) {
                last.pop();
                soft = true;
            }
        }
    }
    out
}

/// Whether a line's parameters say quoted-printable.
fn qp_line(line: &str) -> bool {
    let head = line.split(':').next().unwrap_or("").to_ascii_uppercase();
    head.contains("QUOTED-PRINTABLE")
}

fn quoted_printable(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn split_unquoted(s: &str, at: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == at && !quoted {
            out.push(&s[start..i]);
            start = i + c.len_utf8();
        }
    }
    out.push(&s[start..]);
    out
}

/// Split on `at` where it is not escaped by a backslash; the pieces keep
/// their escapes.
fn split_escaped(s: &str, at: char) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let last = out.last_mut().expect("never empty");
            last.push(c);
            if let Some(n) = chars.next() {
                last.push(n);
            }
        } else if c == at {
            out.push(String::new());
        } else {
            out.last_mut().expect("never empty").push(c);
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

fn clip(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(max)
        .collect()
}

/// A label as one short lower-case word set: what a TYPE or an
/// `X-ABLabel` said, in the words the page translates where it can.
fn label_word(s: &str) -> String {
    let s = s.trim().trim_start_matches("_$!<").trim_end_matches(">!$_");
    let lower = s.to_lowercase();
    match lower.as_str() {
        "cell" | "mobile" | "iphone" => "mobile".into(),
        "home" | "work" | "fax" | "pager" | "other" | "main" => lower,
        "homepage" => "home".into(),
        "" => String::new(),
        _ => clip(s, 32),
    }
}

/// The label of a line: its group's `X-ABLabel` if it has one, else the
/// first TYPE that names a kind rather than a format.
fn label_of(line: &Line, labels: &[(String, String)]) -> String {
    if !line.group.is_empty() {
        if let Some((_, l)) = labels
            .iter()
            .find(|(g, _)| g.eq_ignore_ascii_case(&line.group))
        {
            return label_word(l);
        }
    }
    const KINDS: [&str; 7] = ["cell", "mobile", "work", "home", "fax", "pager", "main"];
    let types = line.types();
    for k in KINDS {
        if types.iter().any(|t| t == k) {
            return label_word(k);
        }
    }
    if types.iter().any(|t| t == "other") {
        return "other".into();
    }
    String::new()
}

fn card(lines: &[Line]) -> Option<Card> {
    let labels: Vec<(String, String)> = lines
        .iter()
        .filter(|l| l.name == "X-ABLABEL" && !l.group.is_empty())
        .map(|l| (l.group.clone(), l.text()))
        .collect();
    let mut c = Contact::default();
    let mut photo = None;
    for line in lines {
        match line.name.as_str() {
            "FN" => c.name = line.text(),
            "N" => {
                let p = line.parts();
                let at = |i: usize| p.get(i).cloned().unwrap_or_default().replace(',', " ");
                c.n = Name {
                    family: at(0),
                    given: at(1),
                    additional: at(2),
                    prefix: at(3),
                    suffix: at(4),
                };
            }
            "NICKNAME" => {
                if c.nickname.is_empty() {
                    c.nickname = split_escaped(&line.raw, ',')
                        .first()
                        .map(|s| unescape(s))
                        .unwrap_or_default();
                }
            }
            "ORG" => {
                if c.org.is_empty() {
                    let p = line.parts();
                    c.org = p
                        .iter()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                }
            }
            "TITLE" => c.title = line.text(),
            "EMAIL" => c.emails.push(Labelled {
                value: line.text(),
                label: label_of(line, &labels),
            }),
            "TEL" => c.phones.push(Labelled {
                value: line.text(),
                label: label_of(line, &labels),
            }),
            "ADR" => {
                let p = line.parts();
                let at = |i: usize| p.get(i).cloned().unwrap_or_default();
                let street = [at(2), at(1)]
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                c.addresses.push(Address {
                    street,
                    locality: at(3),
                    region: at(4),
                    code: at(5),
                    country: at(6),
                    label: label_of(line, &labels),
                });
            }
            "URL" => {
                let v = line.text();
                if let Some(m) = matrix_from_uri(&v) {
                    if c.matrix.is_empty() {
                        c.matrix = m;
                    }
                } else {
                    c.urls.push(Labelled {
                        value: v,
                        label: label_of(line, &labels),
                    });
                }
            }
            "IMPP" | "X-MATRIX" => {
                if let Some(m) = matrix_from_uri(&line.text()) {
                    if c.matrix.is_empty() {
                        c.matrix = m;
                    }
                }
            }
            "X-LOSOS-MESH" => c.mesh = line.text(),
            "BDAY" => c.birthday = line.text(),
            "NOTE" => {
                if c.note.is_empty() {
                    c.note = line.text();
                }
            }
            "UID" => c.uid = line.text(),
            "PHOTO" if photo.is_none() => photo = photo_of(line),
            _ => {}
        }
    }
    c.tidy().then_some(Card { contact: c, photo })
}

/// The picture bytes of a `PHOTO` line: base64 inline (3.0's `ENCODING=b`,
/// 2.1's `ENCODING=BASE64`) or a 4.0 `data:` URI. A photo by URL is left
/// out: the box does not fetch what a file points at.
fn photo_of(line: &Line) -> Option<Vec<u8>> {
    let raw = line.raw.trim();
    let b64 = if let Some(rest) = raw.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',')?;
        if !meta.ends_with(";base64") || !meta.starts_with("image/") {
            return None;
        }
        data
    } else if line
        .param("ENCODING")
        .is_some_and(|e| e.eq_ignore_ascii_case("b") || e.eq_ignore_ascii_case("BASE64"))
    {
        raw
    } else {
        return None;
    };
    let clean: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() > MAX_PHOTO_IN / 3 * 4 + 4 {
        return None;
    }
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(clean.trim_end_matches('='))
        .ok()
}

/// A Matrix user ID, `@local:server`, when `s` is one: lower-case local
/// part of the characters Matrix allows, a server name with no spaces.
pub fn matrix_id(s: &str) -> Option<String> {
    let s = s.trim();
    let rest = s.strip_prefix('@')?;
    let (local, server) = rest.split_once(':')?;
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._=-/+".contains(c));
    let server_ok = !server.is_empty()
        && server
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c));
    (local_ok && server_ok && s.len() <= 255).then(|| s.to_string())
}

/// A Matrix user ID from the ways a card may write one: `matrix:u/name:server`
/// (RFC 9420-era `matrix:` URIs), `matrix:@name:server`, an
/// `https://matrix.to/#/@name:server` link, or the bare ID.
pub fn matrix_from_uri(s: &str) -> Option<String> {
    let s = s.trim();
    let id = if let Some(rest) = s.strip_prefix("matrix:") {
        let rest = rest.split(['?', '#']).next().unwrap_or("");
        match rest.strip_prefix("u/") {
            Some(u) => format!("@{u}"),
            None => rest.to_string(),
        }
    } else if let Some(rest) = s
        .strip_prefix("https://matrix.to/#/")
        .or_else(|| s.strip_prefix("http://matrix.to/#/"))
    {
        rest.split('?').next().unwrap_or("").to_string()
    } else {
        s.to_string()
    };
    let id = id
        .replace("%40", "@")
        .replace("%3A", ":")
        .replace("%3a", ":");
    matrix_id(&id)
}

/// A mesh address, `name@box`, as LosOS Cloud's mesh sharing takes it.
pub fn mesh_address(s: &str) -> Option<String> {
    let s = s.trim();
    let (name, host) = s.split_once('@')?;
    let name_ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c));
    (name_ok && host_ok).then(|| s.to_ascii_lowercase())
}

/// A birthday as `YYYY-MM-DD` or `--MM-DD`, from what cards write:
/// `1990-01-02`, `19900102`, `--0102`, `--01-02`, with or without a time.
pub fn birthday(s: &str) -> Option<String> {
    let s = s.trim();
    let date = s.split('T').next().unwrap_or("");
    let digits: String = date.chars().filter(char::is_ascii_digit).collect();
    let valid = |m: &str, d: &str| {
        let (m, d) = (m.parse::<u32>().ok()?, d.parse::<u32>().ok()?);
        ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some(())
    };
    if date.starts_with("--") && digits.len() == 4 {
        valid(&digits[..2], &digits[2..])?;
        return Some(format!("--{}-{}", &digits[..2], &digits[2..]));
    }
    if digits.len() == 8 && !date.starts_with('-') {
        valid(&digits[4..6], &digits[6..])?;
        return Some(format!(
            "{}-{}-{}",
            &digits[..4],
            &digits[4..6],
            &digits[6..]
        ));
    }
    None
}

/// Which vCard a file is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V3,
    V4,
}

/// One contact to write: the contact, its `UID`, its last change in
/// milliseconds, and its photo as JPEG.
pub struct Out<'a> {
    pub contact: &'a Contact,
    pub uid: &'a str,
    pub updated_ms: i64,
    pub photo: Option<&'a [u8]>,
}

fn type_of(label: &str, v: Version) -> String {
    let t = match label {
        "mobile" => "cell",
        "" => return String::new(),
        l if l.chars().all(|c| c.is_ascii_alphanumeric()) => l,
        // A free-form label is a TYPE value of its own only in 4.0's
        // grammar if it is a word; anything else is left out.
        _ => return String::new(),
    };
    match v {
        Version::V4 => format!(";TYPE={t}"),
        Version::V3 => format!(";TYPE={}", t.to_ascii_uppercase()),
    }
}

/// Fold a content line at 75 octets, never inside a character.
fn fold(line: &str, out: &mut String) {
    let mut width = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += len;
    }
    out.push_str("\r\n");
}

/// The cards for `contacts`, as one file.
pub fn write(contacts: &[Out<'_>], v: Version) -> String {
    let mut out = String::new();
    for o in contacts {
        let c = o.contact;
        let mut lines: Vec<String> = vec![
            "BEGIN:VCARD".into(),
            match v {
                Version::V3 => "VERSION:3.0".into(),
                Version::V4 => "VERSION:4.0".into(),
            },
        ];
        if v == Version::V4 {
            lines.push("KIND:individual".into());
        }
        lines.push(format!("FN:{}", escape(&c.name)));
        let n = &c.n;
        lines.push(format!(
            "N:{};{};{};{};{}",
            escape(&n.family),
            escape(&n.given),
            escape(&n.additional),
            escape(&n.prefix),
            escape(&n.suffix)
        ));
        if !c.nickname.is_empty() {
            lines.push(format!("NICKNAME:{}", escape(&c.nickname)));
        }
        if !c.org.is_empty() {
            lines.push(format!("ORG:{}", escape(&c.org)));
        }
        if !c.title.is_empty() {
            lines.push(format!("TITLE:{}", escape(&c.title)));
        }
        for e in &c.emails {
            let t = match v {
                Version::V3 => format!(
                    ";TYPE=INTERNET{}",
                    type_of(&e.label, v).replacen(";TYPE=", ",", 1)
                ),
                Version::V4 => type_of(&e.label, v),
            };
            lines.push(format!("EMAIL{t}:{}", escape(&e.value)));
        }
        for p in &c.phones {
            lines.push(format!("TEL{}:{}", type_of(&p.label, v), escape(&p.value)));
        }
        for a in &c.addresses {
            let mut street = a.street.splitn(2, '\n');
            let (first, second) = (street.next().unwrap_or(""), street.next().unwrap_or(""));
            lines.push(format!(
                "ADR{}:;{};{};{};{};{};{}",
                type_of(&a.label, v),
                escape(second),
                escape(first),
                escape(&a.locality),
                escape(&a.region),
                escape(&a.code),
                escape(&a.country)
            ));
        }
        for u in &c.urls {
            lines.push(format!("URL{}:{}", type_of(&u.label, v), u.value));
        }
        if !c.birthday.is_empty() {
            let b = match (v, c.birthday.strip_prefix("--")) {
                // 3.0 has no year-less date: leave it out rather than
                // write one a phone reads as the year 0.
                (Version::V3, Some(_)) => String::new(),
                (Version::V4, Some(md)) => format!("--{}", md.replace('-', "")),
                (_, None) => c.birthday.clone(),
            };
            if !b.is_empty() {
                lines.push(format!("BDAY:{b}"));
            }
        }
        if !c.matrix.is_empty() {
            let uri = format!("matrix:u/{}", c.matrix.trim_start_matches('@'));
            match v {
                Version::V4 => lines.push(format!("IMPP;PREF=1:{uri}")),
                Version::V3 => {
                    lines.push(format!("IMPP;X-SERVICE-TYPE=Matrix:{uri}"));
                    lines.push(format!("X-MATRIX:{}", c.matrix));
                }
            }
        }
        if !c.mesh.is_empty() {
            lines.push(format!("X-LOSOS-MESH:{}", c.mesh));
        }
        if !c.note.is_empty() {
            lines.push(format!("NOTE:{}", escape(&c.note)));
        }
        if let Some(jpeg) = o.photo {
            let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
            match v {
                Version::V4 => lines.push(format!("PHOTO:data:image/jpeg;base64,{b64}")),
                Version::V3 => lines.push(format!("PHOTO;ENCODING=b;TYPE=JPEG:{b64}")),
            }
        }
        lines.push(format!("UID:{}", o.uid));
        if let Some(rev) = chrono::DateTime::from_timestamp_millis(o.updated_ms) {
            lines.push(format!("REV:{}", rev.format("%Y%m%dT%H%M%SZ")));
        }
        lines.push("END:VCARD".into());
        for l in lines {
            fold(&l, &mut out);
        }
    }
    out
}

/// Whether [`card`] reads this line into a [`Contact`] (or the card's
/// frame), so that a rewrite writes it anew rather than keeping it.
fn is_read(l: &Line) -> bool {
    match l.name.as_str() {
        "BEGIN" | "END" | "VERSION" | "PRODID" | "REV" | "KIND" | "FN" | "N" | "NICKNAME"
        | "ORG" | "TITLE" | "EMAIL" | "TEL" | "ADR" | "LABEL" | "URL" | "X-LOSOS-MESH" | "BDAY"
        | "NOTE" | "UID" | "PHOTO" => true,
        "IMPP" | "X-MATRIX" => matrix_from_uri(&l.text()).is_some(),
        _ => false,
    }
}

/// One card for `o`, written over `original` (the card as a phone or a
/// desktop address book stored it): every line [`parse`] reads is written
/// anew from `o`, and every other line of `original`'s first card is kept
/// as it was, so what the Contacts page has no field for (categories,
/// phonetic names, other messengers, an anniversary) survives an edit made
/// there. An Apple `X-ABLabel` goes with the line its group labelled.
/// With `keep_photo` the original `PHOTO` line stays too and `o.photo` is
/// not written; quoted-printable lines (2.1, which 3.0 and 4.0 forbid) are
/// dropped.
pub fn rewrite(original: &str, o: &Out<'_>, v: Version, keep_photo: bool) -> String {
    let mut lines: Vec<(String, Line)> = Vec::new();
    let mut depth = 0usize;
    for raw in unfold(original.strip_prefix('\u{feff}').unwrap_or(original)) {
        let Some(line) = Line::read(&raw) else {
            continue;
        };
        let value = line.raw.trim().to_ascii_uppercase();
        if line.name == "BEGIN" && value == "VCARD" {
            depth += 1;
        } else if line.name == "END" && value == "VCARD" {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                break;
            }
        } else if depth == 1 {
            lines.push((raw, line));
        }
    }
    let photo_kept = |l: &Line| keep_photo && l.name == "PHOTO";
    let labelled: std::collections::HashSet<&str> = lines
        .iter()
        .filter(|(_, l)| !l.group.is_empty() && is_read(l) && !photo_kept(l))
        .map(|(_, l)| l.group.as_str())
        .collect();
    let mut extra = String::new();
    for (raw, l) in &lines {
        let drop = (is_read(l) && !photo_kept(l))
            || (l.name == "X-ABLABEL" && labelled.contains(l.group.as_str()))
            || l.param("ENCODING")
                .is_some_and(|e| e.eq_ignore_ascii_case("QUOTED-PRINTABLE"));
        if !drop {
            fold(raw, &mut extra);
        }
    }
    let fresh = Out {
        photo: if keep_photo { None } else { o.photo },
        ..*o
    };
    let mut card = write(std::slice::from_ref(&fresh), v);
    let end = card.len() - "END:VCARD\r\n".len();
    card.insert_str(end, &extra);
    card
}

/// The version a stored card is written in: 4.0 when it says so, 3.0
/// for everything else (2.1 is not kept as it was).
pub fn version_of(text: &str) -> Version {
    let four = unfold(text).iter().any(|l| {
        let l = l.trim().to_ascii_uppercase();
        l == "VERSION:4.0"
    });
    if four {
        Version::V4
    } else {
        Version::V3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(s: &str) -> Card {
        let p = parse(s.as_bytes());
        assert_eq!(p.cards.len(), 1, "{p:?}");
        p.cards.into_iter().next().unwrap()
    }

    #[test]
    fn reads_a_vcard_4() {
        let c = one(concat!(
            "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Eva Nováková\r\nN:Nováková;Eva;;;\r\n",
            "EMAIL;TYPE=work:eva@example.org\r\nTEL;VALUE=uri;TYPE=\"cell,voice\":tel:+421-900-123-456\r\n",
            "ADR;TYPE=home:;;Hlavná 1;Košice;;040 01;Slovakia\r\n",
            "BDAY:--0412\r\nIMPP;PREF=1:matrix:u/eva:kitchenbox.local\r\n",
            "NOTE:Line one\\nLine two\\, with a comma\r\n",
            "UID:urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1\r\nEND:VCARD\r\n"
        ))
        .contact;
        assert_eq!(c.name, "Eva Nováková");
        assert_eq!(c.n.given, "Eva");
        assert_eq!(
            c.emails[0],
            Labelled {
                value: "eva@example.org".into(),
                label: "work".into()
            }
        );
        assert_eq!(
            c.phones[0],
            Labelled {
                value: "+421-900-123-456".into(),
                label: "mobile".into()
            }
        );
        assert_eq!(c.addresses[0].locality, "Košice");
        assert_eq!(c.addresses[0].street, "Hlavná 1");
        assert_eq!(c.birthday, "--04-12");
        assert_eq!(c.matrix, "@eva:kitchenbox.local");
        assert_eq!(c.note, "Line one\nLine two, with a comma");
        assert_eq!(c.uid, "urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1");
    }

    #[test]
    fn reads_a_vcard_3_with_folding_groups_and_a_photo() {
        let jpeg = base64::engine::general_purpose::STANDARD.encode(b"\xFF\xD8\xFFpicture");
        let file = format!(
            concat!(
                "BEGIN:VCARD\nVERSION:3.0\nN:Doe;John;Q.;Dr.;Jr.\nFN:Dr. John Q. Doe Jr.\n",
                "ORG:Example Ltd.;Sales\nTITLE:Seller\n",
                "item1.EMAIL;type=INTERNET;type=pref:john@exa\n mple.com\nitem1.X-ABLabel:_$!<Other>!$_\n",
                "item2.TEL:+1 555 0100\nitem2.X-ABLabel:Boat\n",
                "TEL;TYPE=HOME,VOICE:+1 555 0101\n",
                "BDAY;VALUE=date:1980-02-29\nURL:https://matrix.to/#/@john:example.com\n",
                "PHOTO;ENCODING=b;TYPE=JPEG:{}\nEND:VCARD\n"
            ),
            jpeg
        );
        let card = one(&file);
        let c = card.contact;
        assert_eq!(c.name, "Dr. John Q. Doe Jr.");
        assert_eq!(c.org, "Example Ltd., Sales");
        assert_eq!(
            c.emails[0],
            Labelled {
                value: "john@example.com".into(),
                label: "other".into()
            }
        );
        assert_eq!(
            c.phones[0],
            Labelled {
                value: "+1 555 0100".into(),
                label: "Boat".into()
            }
        );
        assert_eq!(c.phones[1].label, "home");
        assert_eq!(c.birthday, "1980-02-29");
        assert_eq!(c.matrix, "@john:example.com");
        assert!(c.urls.is_empty());
        assert_eq!(card.photo.as_deref(), Some(&b"\xFF\xD8\xFFpicture"[..]));
    }

    #[test]
    fn reads_vcard_2_1_quoted_printable_and_bare_types() {
        let c = one(concat!(
            "BEGIN:VCARD\r\nVERSION:2.1\r\n",
            "N;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:=C5=A0tefan;J=C3=A1n;;;\r\n",
            "FN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:J=C3=A1n =C5=A0te=\r\nfan\r\n",
            "TEL;CELL;PREF:0900111222\r\nEMAIL;HOME:jan@example.sk\r\nEND:VCARD\r\n"
        ))
        .contact;
        assert_eq!(c.name, "Ján Štefan");
        assert_eq!(c.n.family, "Štefan");
        assert_eq!(c.phones[0].label, "mobile");
        assert_eq!(c.emails[0].label, "home");
    }

    #[test]
    fn many_cards_in_one_file_and_empty_ones_are_counted() {
        let file = concat!(
            "BEGIN:VCARD\nVERSION:3.0\nFN:One\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:3.0\nN:;;;;\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:4.0\nEMAIL:three@example.org\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:2.1\nFN:Four\nAGENT:\nBEGIN:VCARD\nFN:Agent\nEND:VCARD\nEND:VCARD\n",
        );
        let p = parse(file.as_bytes());
        let names: Vec<_> = p.cards.iter().map(|c| c.contact.name.as_str()).collect();
        assert_eq!(names, ["One", "three@example.org", "Four"]);
        assert_eq!(p.skipped, 1);
    }

    #[test]
    fn hostile_values_are_bounded() {
        let long = "x".repeat(10_000);
        let mut file = format!("BEGIN:VCARD\nVERSION:4.0\nFN:{long}\nNOTE:{long}\n");
        for i in 0..100 {
            file.push_str(&format!("EMAIL:a{i}@example.org\n"));
        }
        file.push_str("EMAIL:not an address\nURL:javascript:alert(1)\nIMPP:matrix:u/Bad Name:x\nBDAY:2020-13-40\nEND:VCARD\n");
        let c = one(&file).contact;
        assert_eq!(c.name.chars().count(), MAX_FIELD);
        assert_eq!(c.note.chars().count(), MAX_NOTE);
        assert_eq!(c.emails.len(), MAX_LIST);
        assert!(c.urls.is_empty());
        assert!(c.matrix.is_empty());
        assert!(c.birthday.is_empty());
    }

    #[test]
    fn writes_what_it_reads_in_both_versions() {
        let mut c = Contact {
            name: "Eva Nováková; the one, with commas".into(),
            n: Name { family: "Nováková".into(), given: "Eva".into(), ..Name::default() },
            org: "Kitchen".into(),
            emails: vec![Labelled { value: "eva@example.org".into(), label: "work".into() }],
            phones: vec![Labelled { value: "+421900123456".into(), label: "mobile".into() }],
            addresses: vec![Address { street: "Hlavná 1".into(), locality: "Košice".into(), country: "Slovakia".into(), label: "home".into(), ..Address::default() }],
            urls: vec![Labelled { value: "https://eva.example.org".into(), label: String::new() }],
            birthday: "1990-04-12".into(),
            note: "A note that is long enough to be folded across more than one line of the file, with ž and č.\nSecond line.".into(),
            matrix: "@eva:kitchenbox.local".into(),
            mesh: "eva@kitchenbox.local".into(),
            ..Contact::default()
        };
        assert!(c.tidy());
        for v in [Version::V3, Version::V4] {
            let text = write(
                &[Out {
                    contact: &c,
                    uid: "urn:uuid:1",
                    updated_ms: 1_700_000_000_000,
                    photo: Some(b"jpeg"),
                }],
                v,
            );
            assert!(text.lines().all(|l| l.len() <= 76), "{text}");
            let back = one(&text);
            let mut want = c.clone();
            want.uid = "urn:uuid:1".into();
            assert_eq!(back.contact, want, "{v:?}\n{text}");
            assert_eq!(back.photo.as_deref(), Some(&b"jpeg"[..]));
        }
    }

    #[test]
    fn a_year_less_birthday_is_left_out_of_3_0() {
        let c = Contact {
            name: "A".into(),
            birthday: "--04-12".into(),
            ..Contact::default()
        };
        let out = |v| {
            write(
                &[Out {
                    contact: &c,
                    uid: "u",
                    updated_ms: 0,
                    photo: None,
                }],
                v,
            )
        };
        assert!(!out(Version::V3).contains("BDAY"));
        assert!(out(Version::V4).contains("BDAY:--0412"));
        assert_eq!(one(&out(Version::V4)).contact.birthday, "--04-12");
    }

    #[test]
    fn matrix_ids_from_every_spelling() {
        for s in [
            "@eva:box.local",
            "matrix:u/eva:box.local",
            "matrix:u/eva:box.local?action=chat",
            "https://matrix.to/#/@eva:box.local",
            "https://matrix.to/#/%40eva%3Abox.local",
        ] {
            assert_eq!(matrix_from_uri(s).as_deref(), Some("@eva:box.local"), "{s}");
        }
        for s in [
            "eva:box.local",
            "@Eva:box",
            "@eva",
            "@:box",
            "xmpp:eva@box",
            "@eva:box local",
        ] {
            assert_eq!(matrix_from_uri(s), None, "{s}");
        }
    }

    #[test]
    fn mesh_addresses() {
        assert_eq!(mesh_address(" Eva@KitchenBox.local ").as_deref(), None);
        assert_eq!(
            mesh_address("eva@KitchenBox.local").as_deref(),
            Some("eva@kitchenbox.local")
        );
        assert_eq!(mesh_address("eva@"), None);
        assert_eq!(mesh_address("e va@box"), None);
    }
}
