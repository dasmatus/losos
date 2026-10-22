//! iCalendar (RFC 5545) in and out of LosOS Calendar
//! (`crate::cloud::calendar`).
//!
//! The calendar objects themselves stay the text a client stored
//! (`crate::cloud::pim`); this module reads what the Calendar page shows
//! out of them, writes the events the page makes, and expands the ones
//! that repeat.
//!
//! Times are kept the way the file wrote them: a date (all day), a UTC
//! moment, a wall-clock time in a named zone (`TZID`), or a floating wall
//! clock. Turning a zoned wall clock into a moment needs the zone's rules,
//! which the browser has (`Intl`) and the box's cloud does not, so the
//! page gets wall clocks with their zone and turns them into moments
//! itself. Repeats are expanded here, in wall-clock time, which is what
//! RFC 5545 asks for (a weekly 08:00 stays 08:00 across a change of
//! summer time). Where a wall clock meets a UTC bound (`UNTIL`, the query
//! window) the difference is at most a day of zone offset, so the window
//! is widened by [`ZONE_SLACK`] and the page trims what lies outside.
//!
//! Events the page writes name their zone by its IANA name and carry no
//! `VTIMEZONE`, as RFC 7809 allows: phones, Thunderbird and the big
//! calendar services all know those names.
//!
//! [`split`] cuts a file of many events (an import, a subscribed feed)
//! into one calendar object per UID, each with the `VTIMEZONE`s it uses.
//! [`request`], [`reply`] and [`with_partstat`] are the iTIP (RFC 5546)
//! messages invitations travel in.
//!
//! Every input is bounded ([`MAX_TEXT`], [`MAX_PEOPLE`], [`MAX_ALARMS`],
//! the expansion's [`MAX_STEPS`]), so a hostile feed cannot make one event
//! cost without end.

use serde::{Deserialize, Serialize};

/// The longest text a summary, place or description keeps, in characters.
pub const MAX_TEXT: usize = 8000;
/// The most attendees an event keeps.
pub const MAX_PEOPLE: usize = 100;
/// The most reminders an event keeps.
pub const MAX_ALARMS: usize = 8;
/// The most periods one expansion walks through.
pub const MAX_STEPS: usize = 20_000;
/// The most occurrences one expansion returns.
pub const MAX_OCCURRENCES: usize = 2000;
/// How far a wall clock in any zone may be from UTC, with room to spare.
pub const ZONE_SLACK: i64 = 86_400;

const PRODID: &str = "-//LosOS//LosOS Calendar//EN";

// ── Civil time ─────────────────────────────────────────────────────────────

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number from [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Monday 0 to Sunday 6.
pub fn weekday(days: i64) -> i64 {
    // 1970-01-01 was a Thursday.
    (days + 3).rem_euclid(7)
}

fn month_len(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// How a time is meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    /// A whole day, `VALUE=DATE`.
    Date,
    /// A wall clock wherever the reader is.
    Floating,
    /// A moment, written with `Z`.
    Utc,
    /// A wall clock in a named zone (`TZID=`).
    Tz(String),
}

/// A time as the file wrote it: seconds of wall clock since 1970-01-01
/// 00:00 (as if that wall clock were UTC), and how they are meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Time {
    pub at: i64,
    pub zone: Zone,
}

impl Time {
    pub fn is_date(&self) -> bool {
        self.zone == Zone::Date
    }

    /// `2026-10-22` for a date, `2026-10-22T08:00:00` otherwise.
    pub fn local(&self) -> String {
        let days = self.at.div_euclid(86_400);
        let secs = self.at.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        if self.is_date() {
            format!("{y:04}-{m:02}-{d:02}")
        } else {
            format!(
                "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
                secs / 3600,
                secs / 60 % 60,
                secs % 60
            )
        }
    }

    /// The zone the page turns the wall clock into a moment with: `UTC`,
    /// an IANA name, or nothing (the reader's own, and dates).
    pub fn zone_name(&self) -> Option<String> {
        match &self.zone {
            Zone::Utc => Some("UTC".into()),
            Zone::Tz(z) => Some(z.clone()),
            Zone::Date | Zone::Floating => None,
        }
    }

    /// The value and parameters of a DTSTART-like line for this time.
    fn property(&self, name: &str) -> String {
        let days = self.at.div_euclid(86_400);
        let secs = self.at.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        let date = format!("{y:04}{m:02}{d:02}");
        let clock = format!("T{:02}{:02}{:02}", secs / 3600, secs / 60 % 60, secs % 60);
        match &self.zone {
            Zone::Date => format!("{name};VALUE=DATE:{date}"),
            Zone::Floating => format!("{name}:{date}{clock}"),
            Zone::Utc => format!("{name}:{date}{clock}Z"),
            Zone::Tz(z) => format!("{name};TZID={}:{date}{clock}", param_value(z)),
        }
    }

    /// From the page: `2026-10-22` or `2026-10-22T08:00[:00]`, and a zone
    /// (`UTC`, an IANA name, or none for a floating wall clock).
    pub fn from_page(local: &str, zone: Option<&str>) -> Option<Time> {
        let (date, clock) = match local.split_once('T') {
            Some((d, c)) => (d, Some(c)),
            None => (local, None),
        };
        let mut parts = date.split('-');
        let y: i64 = parts.next()?.parse().ok()?;
        let m: i64 = parts.next()?.parse().ok()?;
        let d: i64 = parts.next()?.parse().ok()?;
        if parts.next().is_some()
            || !(1..=9999).contains(&y)
            || !(1..=12).contains(&m)
            || d < 1
            || d > month_len(y, m)
        {
            return None;
        }
        let days = days_from_civil(y, m, d);
        let Some(clock) = clock else {
            return Some(Time {
                at: days * 86_400,
                zone: Zone::Date,
            });
        };
        let mut c = clock.split(':');
        let h: i64 = c.next()?.parse().ok()?;
        let mi: i64 = c.next()?.parse().ok()?;
        let s: i64 = c.next().map(str::parse).transpose().ok()?.unwrap_or(0);
        if c.next().is_some() || h > 23 || mi > 59 || s > 59 || h < 0 || mi < 0 || s < 0 {
            return None;
        }
        let zone = match zone.map(str::trim) {
            None | Some("") => Zone::Floating,
            Some("UTC" | "Z" | "Etc/UTC") => Zone::Utc,
            Some(z) if valid_zone(z) => Zone::Tz(z.to_owned()),
            Some(_) => return None,
        };
        Some(Time {
            at: days * 86_400 + h * 3600 + mi * 60 + s,
            zone,
        })
    }
}

/// A zone name the page may hand in: IANA-shaped, nothing that could break
/// a parameter.
pub fn valid_zone(z: &str) -> bool {
    !z.is_empty()
        && z.len() <= 64
        && z.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_-+".contains(c))
}

/// An iCalendar DATE or DATE-TIME, with the TZID or VALUE it came with.
pub fn parse_time(value: &str, params: &[(String, String)]) -> Option<Time> {
    let v = value.trim();
    let digits = |s: &str| s.parse::<i64>().ok();
    let (date, clock) = match v.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (v, None),
    };
    if date.len() != 8 {
        return None;
    }
    let (y, m, d) = (
        digits(&date[..4])?,
        digits(&date[4..6])?,
        digits(&date[6..8])?,
    );
    if !(1..=12).contains(&m) || d < 1 || d > month_len(y, m) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    let Some(clock) = clock else {
        return Some(Time {
            at: days * 86_400,
            zone: Zone::Date,
        });
    };
    let utc = clock.ends_with('Z');
    let clock = clock.trim_end_matches('Z');
    if clock.len() < 6 {
        return None;
    }
    let secs = digits(&clock[..2])? * 3600 + digits(&clock[2..4])? * 60 + digits(&clock[4..6])?;
    let zone = if utc {
        Zone::Utc
    } else {
        match param(params, "TZID") {
            Some(z) => tz_name(z),
            None => Zone::Floating,
        }
    };
    Some(Time {
        at: days * 86_400 + secs,
        zone,
    })
}

/// A TZID as a zone. Outlook and older exporters write their own names or
/// a path prefix; the common ones are turned into IANA names, and
/// anything else is kept for the page to try.
fn tz_name(tzid: &str) -> Zone {
    let t = tzid.trim().trim_start_matches('/');
    // "/mozilla.org/20050126_1/Europe/Bratislava" and the like.
    let t = match t.find(|c: char| c.is_ascii_uppercase()) {
        Some(i) if t[..i].contains('/') => &t[i..],
        _ => t,
    };
    let mapped = match t {
        "UTC" | "GMT" | "Z" | "Etc/UTC" | "Etc/GMT" | "Coordinated Universal Time" => {
            return Zone::Utc
        }
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "GMT Standard Time" => "Europe/London",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "FLE Standard Time" => "Europe/Kyiv",
        "Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "Pacific Standard Time" => "America/Los_Angeles",
        other => other,
    };
    if valid_zone(mapped) {
        Zone::Tz(mapped.to_owned())
    } else {
        Zone::Floating
    }
}

/// `P1DT2H`, `-PT15M` and the like, in seconds.
pub fn parse_duration(value: &str) -> Option<i64> {
    let v = value.trim();
    let (sign, v) = match v.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut num = String::new();
    let mut in_time = false;
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            unit => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match (unit, in_time) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    if !num.is_empty() {
        return None;
    }
    Some(sign * total)
}

// ── Content lines ──────────────────────────────────────────────────────────

/// The content lines of an iCalendar text, unfolded.
pub fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(cont) = line.strip_prefix([' ', '\t']) {
            if let Some(prev) = out.last_mut() {
                prev.push_str(cont);
                continue;
            }
        }
        if !line.trim().is_empty() {
            out.push(line.to_owned());
        }
    }
    out
}

/// One content line: its name (upper case), parameters (names upper case,
/// quotes taken off) and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub value: String,
}

impl Line {
    pub fn parse(line: &str) -> Line {
        // The value starts at the first colon outside quotes.
        let mut quoted = false;
        let mut split = line.len();
        for (i, c) in line.char_indices() {
            match c {
                '"' => quoted = !quoted,
                ':' if !quoted => {
                    split = i;
                    break;
                }
                _ => {}
            }
        }
        let head = &line[..split];
        let value = line.get(split + 1..).unwrap_or("").to_owned();
        let mut parts = Vec::new();
        let mut cur = String::new();
        let mut quoted = false;
        for c in head.chars() {
            match c {
                '"' => {
                    quoted = !quoted;
                    cur.push(c);
                }
                ';' if !quoted => parts.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
        parts.push(cur);
        let mut it = parts.into_iter();
        let name = it.next().unwrap_or_default();
        // A group prefix (`item1.X`) is not ours to read.
        let name = name
            .rsplit('.')
            .next()
            .unwrap_or(&name)
            .to_ascii_uppercase();
        let params = it
            .filter_map(|p| {
                let (k, v) = p.split_once('=')?;
                Some((
                    k.trim().to_ascii_uppercase(),
                    v.trim().trim_matches('"').to_owned(),
                ))
            })
            .collect();
        Line {
            name,
            params,
            value,
        }
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        param(&self.params, name)
    }

    /// The line as text again, unfolded.
    pub fn text(&self) -> String {
        let mut out = self.name.clone();
        for (k, v) in &self.params {
            out.push(';');
            out.push_str(k);
            out.push('=');
            out.push_str(&param_value(v));
        }
        out.push(':');
        out.push_str(&self.value);
        out
    }

    fn set_param(&mut self, name: &str, value: &str) {
        match self.params.iter_mut().find(|(k, _)| k == name) {
            Some(p) => p.1 = value.to_owned(),
            None => self.params.push((name.to_owned(), value.to_owned())),
        }
    }
}

fn param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// A parameter value, quoted when it holds what would end it.
fn param_value(v: &str) -> String {
    let v: String = v.chars().filter(|c| *c != '"' && !c.is_control()).collect();
    if v.contains([':', ';', ',']) {
        format!("\"{v}\"")
    } else {
        v
    }
}

/// A TEXT value as it reads: `\n`, `\,`, `\;` and `\\` undone.
pub fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A TEXT value as it is written.
pub fn escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

fn bounded(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

/// A line folded at 75 octets, never inside a character, with CRLF.
pub fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + line.len() / 70 * 3 + 2);
    let mut width = 0;
    for c in line.chars() {
        let n = c.len_utf8();
        if width + n > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += n;
    }
    out.push_str("\r\n");
    out
}

fn join(lines: &[String]) -> String {
    lines.iter().map(|l| fold(l)).collect()
}

// ── Reading ────────────────────────────────────────────────────────────────

/// Someone an event names: its organizer or an attendee.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Person {
    pub email: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// `NEEDS-ACTION`, `ACCEPTED`, `DECLINED`, `TENTATIVE`, `DELEGATED`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// Their Matrix ID, when the page knew it (`X-LOSOS-MATRIX`), for a
    /// chat or a call with them.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub matrix: String,
}

impl Person {
    fn from_line(l: &Line) -> Option<Person> {
        let v = l.value.trim();
        let email = v
            .strip_prefix("mailto:")
            .or_else(|| v.strip_prefix("MAILTO:"))
            .unwrap_or(v);
        if email.is_empty() || !email.contains('@') {
            return None;
        }
        Some(Person {
            email: bounded(email, 254).to_ascii_lowercase(),
            name: bounded(l.param("CN").unwrap_or(""), 200),
            status: l
                .param("PARTSTAT")
                .unwrap_or("")
                .to_ascii_uppercase()
                .chars()
                .take(20)
                .collect(),
            matrix: bounded(l.param("X-LOSOS-MATRIX").unwrap_or(""), 255),
        })
    }

    fn line(&self, name: &str, attendee: bool) -> String {
        let mut l = Line {
            name: name.to_owned(),
            params: Vec::new(),
            value: format!("mailto:{}", self.email),
        };
        if !self.name.is_empty() {
            l.params.push(("CN".into(), self.name.clone()));
        }
        if attendee {
            l.params.push(("ROLE".into(), "REQ-PARTICIPANT".into()));
            let status = if self.status.is_empty() {
                "NEEDS-ACTION"
            } else {
                &self.status
            };
            l.params.push(("PARTSTAT".into(), status.to_owned()));
            if status == "NEEDS-ACTION" {
                l.params.push(("RSVP".into(), "TRUE".into()));
            }
        }
        if !self.matrix.is_empty() {
            l.params
                .push(("X-LOSOS-MATRIX".into(), self.matrix.clone()));
        }
        l.text()
    }
}

/// How often an event repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// An RRULE, the parts of it this module expands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub freq: Freq,
    pub interval: i64,
    pub count: Option<i64>,
    pub until: Option<Time>,
    /// (ordinal, weekday): ordinal 0 for every one, 1 the first, -1 the
    /// last; Monday 0.
    pub by_day: Vec<(i64, i64)>,
    pub by_month_day: Vec<i64>,
    pub by_month: Vec<i64>,
    pub by_set_pos: Vec<i64>,
}

const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

impl Rule {
    /// `None` for what this module cannot expand (hourly and finer); the
    /// event then shows once, at its start.
    pub fn parse(v: &str) -> Option<Rule> {
        let mut rule = Rule {
            freq: Freq::Daily,
            interval: 1,
            count: None,
            until: None,
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
        };
        let mut freq = None;
        for part in v.split(';') {
            let Some((k, val)) = part.split_once('=') else {
                continue;
            };
            let list = || val.split(',').map(str::trim).filter(|s| !s.is_empty());
            match k.trim().to_ascii_uppercase().as_str() {
                "FREQ" => {
                    freq = Some(match val.trim().to_ascii_uppercase().as_str() {
                        "DAILY" => Freq::Daily,
                        "WEEKLY" => Freq::Weekly,
                        "MONTHLY" => Freq::Monthly,
                        "YEARLY" => Freq::Yearly,
                        _ => return None,
                    })
                }
                "INTERVAL" => rule.interval = val.trim().parse().ok().filter(|n| *n >= 1)?,
                "COUNT" => rule.count = Some(val.trim().parse().ok().filter(|n| *n >= 1)?),
                "UNTIL" => rule.until = parse_time(val, &[]),
                "BYDAY" => {
                    for d in list().take(31) {
                        let d = d.to_ascii_uppercase();
                        let (ord, name) = d.split_at(d.len().saturating_sub(2));
                        let wd = DAYS.iter().position(|x| *x == name)? as i64;
                        let ord = if ord.is_empty() {
                            0
                        } else {
                            ord.trim_start_matches('+').parse().ok()?
                        };
                        rule.by_day.push((ord, wd));
                    }
                }
                "BYMONTHDAY" => {
                    rule.by_month_day = list()
                        .take(31)
                        .filter_map(|d| d.parse().ok())
                        .filter(|d: &i64| *d != 0 && d.abs() <= 31)
                        .collect()
                }
                "BYMONTH" => {
                    rule.by_month = list()
                        .take(12)
                        .filter_map(|d| d.parse().ok())
                        .filter(|d| (1..=12).contains(d))
                        .collect()
                }
                "BYSETPOS" => {
                    rule.by_set_pos = list()
                        .take(31)
                        .filter_map(|d| d.parse().ok())
                        .filter(|d: &i64| *d != 0 && d.abs() <= 366)
                        .collect()
                }
                _ => {}
            }
        }
        rule.freq = freq?;
        Some(rule)
    }

    /// The RRULE value again.
    pub fn text(&self) -> String {
        let mut out = format!(
            "FREQ={}",
            match self.freq {
                Freq::Daily => "DAILY",
                Freq::Weekly => "WEEKLY",
                Freq::Monthly => "MONTHLY",
                Freq::Yearly => "YEARLY",
            }
        );
        if self.interval > 1 {
            out.push_str(&format!(";INTERVAL={}", self.interval));
        }
        if let Some(c) = self.count {
            out.push_str(&format!(";COUNT={c}"));
        }
        if let Some(u) = &self.until {
            let p = u.property("X");
            out.push_str(&format!(";UNTIL={}", p.rsplit(':').next().unwrap_or("")));
        }
        if !self.by_day.is_empty() {
            let days: Vec<String> = self
                .by_day
                .iter()
                .map(|(o, d)| {
                    let name = DAYS[*d as usize];
                    if *o == 0 {
                        name.to_owned()
                    } else {
                        format!("{o}{name}")
                    }
                })
                .collect();
            out.push_str(&format!(";BYDAY={}", days.join(",")));
        }
        let nums = |v: &[i64]| v.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        if !self.by_month_day.is_empty() {
            out.push_str(&format!(";BYMONTHDAY={}", nums(&self.by_month_day)));
        }
        if !self.by_month.is_empty() {
            out.push_str(&format!(";BYMONTH={}", nums(&self.by_month)));
        }
        if !self.by_set_pos.is_empty() {
            out.push_str(&format!(";BYSETPOS={}", nums(&self.by_set_pos)));
        }
        out
    }
}

/// One VEVENT, what the Calendar page reads of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub uid: String,
    pub recurrence_id: Option<Time>,
    pub summary: String,
    pub location: String,
    pub description: String,
    pub start: Time,
    /// How long it lasts, in seconds of wall clock.
    pub length: i64,
    pub rule: Option<Rule>,
    /// The RRULE as written, also when [`Rule::parse`] could not read it.
    pub rrule: Option<String>,
    pub exdates: Vec<Time>,
    pub rdates: Vec<Time>,
    /// `CONFIRMED`, `TENTATIVE`, `CANCELLED`, or empty.
    pub status: String,
    pub organizer: Option<Person>,
    pub attendees: Vec<Person>,
    /// Reminders, in seconds before the start (negative: after it).
    pub alarms: Vec<i64>,
    pub url: String,
    /// `video` or `voice` when the event is held as a call in LosOS Chat
    /// (`X-LOSOS-CALL`).
    pub call: String,
    pub sequence: i64,
}

/// What a calendar file holds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Calendar {
    /// iTIP's `METHOD` (`REQUEST`, `REPLY`, `CANCEL`), upper case.
    pub method: String,
    /// `X-WR-CALNAME`, what a feed calls itself.
    pub name: String,
    /// `COLOR` (RFC 7986) or `X-APPLE-CALENDAR-COLOR`.
    pub color: String,
    /// How often the feed asks to be read again (`REFRESH-INTERVAL`,
    /// `X-PUBLISHED-TTL`), in seconds.
    pub refresh: Option<i64>,
    pub events: Vec<Event>,
}

/// The components of a file at the top level, each as its lines, with its
/// name (`VEVENT`, `VTIMEZONE`, …).
fn components(lines: &[String]) -> (Vec<String>, Vec<(String, Vec<String>)>) {
    let mut head = Vec::new();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut depth = 0usize;
    for l in lines {
        let parsed = Line::parse(l);
        match parsed.name.as_str() {
            "BEGIN" => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
                if depth == 2 {
                    out.push((parsed.value.trim().to_ascii_uppercase(), vec![l.clone()]));
                    continue;
                }
            }
            "END" => {
                if depth == 1 {
                    depth = 0;
                    continue;
                }
                if depth == 2 {
                    if let Some(last) = out.last_mut() {
                        last.1.push(l.clone());
                    }
                    depth -= 1;
                    continue;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        if depth >= 2 {
            if let Some(last) = out.last_mut() {
                last.1.push(l.clone());
            }
        } else if depth == 1 {
            head.push(l.clone());
        }
    }
    (head, out)
}

/// One VEVENT's lines as an [`Event`]; `None` without a DTSTART.
fn event(lines: &[String]) -> Option<Event> {
    let mut e = Event {
        uid: String::new(),
        recurrence_id: None,
        summary: String::new(),
        location: String::new(),
        description: String::new(),
        start: Time {
            at: 0,
            zone: Zone::Floating,
        },
        length: 0,
        rule: None,
        rrule: None,
        exdates: Vec::new(),
        rdates: Vec::new(),
        status: String::new(),
        organizer: None,
        attendees: Vec::new(),
        alarms: Vec::new(),
        url: String::new(),
        call: String::new(),
        sequence: 0,
    };
    let mut start = None;
    let mut end = None;
    let mut duration = None;
    let mut depth = 0usize;
    let mut in_alarm = false;
    for raw in lines {
        let l = Line::parse(raw);
        match l.name.as_str() {
            "BEGIN" => {
                depth += 1;
                in_alarm = depth == 2 && l.value.trim().eq_ignore_ascii_case("VALARM");
                continue;
            }
            "END" => {
                depth = depth.saturating_sub(1);
                if depth < 2 {
                    in_alarm = false;
                }
                continue;
            }
            _ => {}
        }
        if in_alarm {
            if l.name == "TRIGGER" && e.alarms.len() < MAX_ALARMS {
                // Relative to the start (the usual) or to the end.
                if let Some(d) = parse_duration(&l.value) {
                    if l.param("RELATED")
                        .is_none_or(|r| r.eq_ignore_ascii_case("START"))
                    {
                        e.alarms.push(-d);
                    }
                } else if let Some(t) = parse_time(&l.value, &[]) {
                    // An absolute trigger: kept relative to the start.
                    if let Some(s) = &start {
                        let s: &Time = s;
                        e.alarms.push(s.at - t.at);
                    }
                }
            }
            continue;
        }
        if depth != 1 {
            continue;
        }
        match l.name.as_str() {
            "UID" => e.uid = bounded(&l.value, 255),
            "SUMMARY" => e.summary = bounded(&unescape(&l.value), MAX_TEXT),
            "LOCATION" => e.location = bounded(&unescape(&l.value), MAX_TEXT),
            "DESCRIPTION" => e.description = bounded(&unescape(&l.value), MAX_TEXT),
            "DTSTART" => start = parse_time(&l.value, &l.params),
            "DTEND" => end = parse_time(&l.value, &l.params),
            "DURATION" => duration = parse_duration(&l.value),
            "RECURRENCE-ID" => e.recurrence_id = parse_time(&l.value, &l.params),
            "RRULE" => {
                e.rrule = Some(bounded(&l.value, 500));
                e.rule = Rule::parse(&l.value);
            }
            "EXDATE" | "RDATE" => {
                let list = if l.name == "EXDATE" {
                    &mut e.exdates
                } else {
                    &mut e.rdates
                };
                for v in l.value.split(',').take(1000) {
                    if let Some(t) = parse_time(v, &l.params) {
                        list.push(t);
                    }
                }
            }
            "STATUS" => e.status = bounded(&l.value, 20).to_ascii_uppercase(),
            "ORGANIZER" => e.organizer = Person::from_line(&l),
            "ATTENDEE" if e.attendees.len() < MAX_PEOPLE => {
                if let Some(p) = Person::from_line(&l) {
                    e.attendees.push(p);
                }
            }
            "URL" => e.url = bounded(&l.value, 2000),
            "X-LOSOS-CALL" => {
                let v = l.value.trim().to_ascii_lowercase();
                if v == "video" || v == "voice" {
                    e.call = v;
                }
            }
            "SEQUENCE" => e.sequence = l.value.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    let start = start?;
    // A date start's end is a date too; with neither, a date lasts a day
    // and a time is a moment.
    e.length = match (end, duration) {
        (Some(end), _) if end.at > start.at => end.at - start.at,
        (_, Some(d)) if d > 0 => d,
        _ if start.is_date() => 86_400,
        _ => 0,
    };
    e.start = start;
    Some(e)
}

/// Read a calendar file. Lenient: what it cannot read it skips.
pub fn parse(text: &str) -> Calendar {
    let lines = unfold(text);
    let mut cal = Calendar::default();
    let Some(first) = lines.first() else {
        return cal;
    };
    if !first.trim().eq_ignore_ascii_case("BEGIN:VCALENDAR") {
        return cal;
    }
    let (head, comps) = components(&lines);
    for l in &head {
        let l = Line::parse(l);
        match l.name.as_str() {
            "METHOD" => cal.method = bounded(&l.value, 20).to_ascii_uppercase(),
            "X-WR-CALNAME" | "NAME" => cal.name = bounded(&unescape(&l.value), 200),
            "COLOR" | "X-APPLE-CALENDAR-COLOR" => cal.color = bounded(&l.value, 40),
            "REFRESH-INTERVAL" | "X-PUBLISHED-TTL" => cal.refresh = parse_duration(&l.value),
            _ => {}
        }
    }
    for (name, lines) in &comps {
        if name == "VEVENT" {
            if let Some(e) = event(lines) {
                cal.events.push(e);
            }
        }
    }
    cal
}

// ── Repeats ────────────────────────────────────────────────────────────────

/// The days (day numbers) a rule's period holds, in order: the period that
/// starts on `days` (a day, the Monday of a week, the first of a month or
/// of a year).
fn period_days(rule: &Rule, freq_start: i64, start_day: i64) -> Vec<i64> {
    let (_, sm, sd) = civil_from_days(start_day);
    let in_month = |y: i64, m: i64| -> Vec<i64> {
        let len = month_len(y, m);
        let first = days_from_civil(y, m, 1);
        let mut out: Vec<i64> = Vec::new();
        if !rule.by_month_day.is_empty() {
            for d in &rule.by_month_day {
                let d = if *d < 0 { len + 1 + d } else { *d };
                if (1..=len).contains(&d) {
                    out.push(first + d - 1);
                }
            }
            if !rule.by_day.is_empty() {
                out.retain(|day| rule.by_day.iter().any(|(_, wd)| weekday(*day) == *wd));
            }
        } else if !rule.by_day.is_empty() {
            for (ord, wd) in &rule.by_day {
                let all: Vec<i64> = (0..len)
                    .map(|i| first + i)
                    .filter(|day| weekday(*day) == *wd)
                    .collect();
                match *ord {
                    0 => out.extend(all),
                    n if n > 0 => out.extend(all.get(n as usize - 1)),
                    n => out.extend(
                        all.len()
                            .checked_sub(n.unsigned_abs() as usize)
                            .and_then(|i| all.get(i)),
                    ),
                }
            }
        } else if sd <= len {
            out.push(first + sd - 1);
        }
        out
    };
    let mut days = match rule.freq {
        Freq::Daily => {
            let ok_day = rule.by_day.is_empty()
                || rule.by_day.iter().any(|(_, wd)| weekday(freq_start) == *wd);
            let (y, m, d) = civil_from_days(freq_start);
            let ok_md = rule.by_month_day.is_empty()
                || rule.by_month_day.contains(&d)
                || rule.by_month_day.contains(&(d - month_len(y, m) - 1));
            if ok_day && ok_md {
                vec![freq_start]
            } else {
                vec![]
            }
        }
        Freq::Weekly => {
            if rule.by_day.is_empty() {
                vec![freq_start + weekday(start_day)]
            } else {
                let mut v: Vec<i64> = rule.by_day.iter().map(|(_, wd)| freq_start + wd).collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        }
        Freq::Monthly => {
            let (y, m, _) = civil_from_days(freq_start);
            in_month(y, m)
        }
        Freq::Yearly => {
            let (y, _, _) = civil_from_days(freq_start);
            let months = if rule.by_month.is_empty() {
                vec![sm]
            } else {
                rule.by_month.clone()
            };
            let mut v = Vec::new();
            for m in months {
                v.extend(in_month(y, m));
            }
            v
        }
    };
    if !rule.by_month.is_empty() && rule.freq != Freq::Yearly {
        days.retain(|day| rule.by_month.contains(&civil_from_days(*day).1));
    }
    days.sort_unstable();
    days.dedup();
    if !rule.by_set_pos.is_empty() {
        let n = days.len() as i64;
        let mut picked: Vec<i64> = rule
            .by_set_pos
            .iter()
            .filter_map(|p| {
                let i = if *p > 0 { p - 1 } else { n + p };
                (0..n).contains(&i).then(|| days[i as usize])
            })
            .collect();
        picked.sort_unstable();
        picked.dedup();
        days = picked;
    }
    days
}

/// The first day of the `k`-th period after the one `start_day` is in.
fn nth_period(freq: Freq, start_day: i64, k: i64) -> i64 {
    let (y, m, _) = civil_from_days(start_day);
    match freq {
        Freq::Daily => start_day + k,
        Freq::Weekly => start_day - weekday(start_day) + 7 * k,
        Freq::Monthly => {
            let months = (y * 12 + (m - 1)) + k;
            days_from_civil(months.div_euclid(12), months.rem_euclid(12) + 1, 1)
        }
        Freq::Yearly => days_from_civil(y + k, 1, 1),
    }
}

/// Whether a candidate start is past the rule's UNTIL.
fn past_until(rule: &Rule, start: &Time, at: i64) -> bool {
    let Some(until) = &rule.until else {
        return false;
    };
    match (&start.zone, &until.zone) {
        // A date against a date, or a time against a time in the same way.
        (Zone::Date, _) => at.div_euclid(86_400) > until.at.div_euclid(86_400),
        (_, Zone::Date) => at.div_euclid(86_400) > until.at.div_euclid(86_400),
        // A wall clock against a UTC moment, with no zone rules to turn
        // one into the other: by the day. UNTIL is the last instance's
        // start or the end of the last day, which for the zones of Europe
        // and Africa falls on that day in UTC too.
        (Zone::Tz(_) | Zone::Floating, Zone::Utc) => {
            at.div_euclid(86_400) > until.at.div_euclid(86_400)
        }
        _ => at > until.at,
    }
}

/// The starts (wall clock, as [`Time::at`]) of an event's occurrences
/// that overlap `[from, to)`, also in wall clock; at most `limit`.
/// Overridden instances (`skip`) and EXDATEs are left out.
pub fn starts(e: &Event, from: i64, to: i64, skip: &[i64], limit: usize) -> Vec<i64> {
    let mut out = Vec::new();
    let len = e.length.max(0);
    let excluded = |at: i64| {
        skip.contains(&at)
            || e.exdates.iter().any(|x| {
                if x.is_date() || e.start.is_date() {
                    x.at.div_euclid(86_400) == at.div_euclid(86_400)
                } else {
                    x.at == at
                }
            })
    };
    let overlaps = |at: i64| at < to && (at + len > from || (len == 0 && at >= from));
    let Some(rule) = &e.rule else {
        if overlaps(e.start.at) && !excluded(e.start.at) {
            out.push(e.start.at);
        }
        for r in &e.rdates {
            if out.len() < limit && r.at != e.start.at && overlaps(r.at) && !excluded(r.at) {
                out.push(r.at);
            }
        }
        out.sort_unstable();
        return out;
    };
    let start_day = e.start.at.div_euclid(86_400);
    let clock = e.start.at.rem_euclid(86_400);
    let step = rule.interval.max(1);
    let mut counted = 0i64;
    let mut k = 0i64;
    let mut steps = 0usize;
    // Jump close to the window when nothing counts occurrences: a weekly
    // event from 2015 should not walk ten years of weeks first.
    if rule.count.is_none() && from - ZONE_SLACK > e.start.at {
        let gap_days = (from - e.start.at) / 86_400;
        // The longest a period can be, so the jump never lands past it.
        let per = match rule.freq {
            Freq::Daily => 1,
            Freq::Weekly => 7,
            Freq::Monthly => 31,
            Freq::Yearly => 366,
        } * step;
        let periods = (gap_days / per - 1).max(0);
        k = periods * step;
    }
    'periods: while steps < MAX_STEPS {
        steps += 1;
        let first = nth_period(rule.freq, start_day, k);
        k += step;
        let days = period_days(rule, first, start_day);
        if first * 86_400 > to + 366 * 86_400 {
            break;
        }
        for day in days {
            let at = day * 86_400 + clock;
            if at < e.start.at {
                continue;
            }
            if past_until(rule, &e.start, at) {
                break 'periods;
            }
            if let Some(c) = rule.count {
                if counted >= c {
                    break 'periods;
                }
            }
            counted += 1;
            if at >= to {
                break 'periods;
            }
            if overlaps(at) && !excluded(at) {
                out.push(at);
                if out.len() >= limit {
                    break 'periods;
                }
            }
        }
    }
    // The first instance is DTSTART even where the rule would not land on
    // it (RFC 5545 counts it either way).
    if !out.contains(&e.start.at)
        && overlaps(e.start.at)
        && !excluded(e.start.at)
        && !period_days(rule, nth_period(rule.freq, start_day, 0), start_day).contains(&start_day)
    {
        out.insert(0, e.start.at);
    }
    for r in &e.rdates {
        if out.len() < limit && !out.contains(&r.at) && overlaps(r.at) && !excluded(r.at) {
            out.push(r.at);
        }
    }
    out.sort_unstable();
    out
}

// ── Writing ────────────────────────────────────────────────────────────────

/// How an event repeats, as the page sets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repeat {
    pub freq: Freq,
    #[serde(default = "one")]
    pub interval: i64,
    /// The last day, `2026-12-31`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    /// `MO`, `TU`, …: the days of the week, for a weekly repeat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<String>,
    /// The event's zone's offset from UTC on the last day, in minutes
    /// east, which the page knows and the box does not: UNTIL is UTC.
    #[serde(default, skip_serializing)]
    pub offset: i64,
}

fn one() -> i64 {
    1
}

impl Repeat {
    /// The repeat of a rule, where the page's fields can say it.
    pub fn of(rule: &Rule) -> Option<Repeat> {
        let plain_days = rule.by_day.iter().all(|(o, _)| *o == 0);
        if !rule.by_month_day.is_empty()
            || !rule.by_month.is_empty()
            || !rule.by_set_pos.is_empty()
            || !plain_days
            || (rule.freq != Freq::Weekly && !rule.by_day.is_empty())
        {
            return None;
        }
        Some(Repeat {
            freq: rule.freq,
            interval: rule.interval,
            until: rule.until.as_ref().map(|u| {
                Time {
                    at: u.at,
                    zone: Zone::Date,
                }
                .local()
            }),
            count: rule.count,
            days: rule
                .by_day
                .iter()
                .map(|(_, d)| DAYS[*d as usize].to_owned())
                .collect(),
            offset: 0,
        })
    }

    fn rule(&self, start: &Time) -> Result<Rule, String> {
        if !(1..=1000).contains(&self.interval) {
            return Err("the repeat's interval is out of range".into());
        }
        let until = match &self.until {
            Some(d) if !d.is_empty() => {
                let day = Time::from_page(d, None)
                    .filter(Time::is_date)
                    .ok_or("the repeat's last day is not a date")?;
                // The end of that day, in the start's own terms.
                Some(match &start.zone {
                    Zone::Date => day,
                    Zone::Utc => Time {
                        at: day.at + 86_399,
                        zone: Zone::Utc,
                    },
                    Zone::Floating => Time {
                        at: day.at + 86_399,
                        zone: Zone::Floating,
                    },
                    // A zoned wall clock's UNTIL is UTC (RFC 5545): the
                    // end of the last day there, kept on that day in UTC
                    // too, which is how it is read back (`past_until`).
                    Zone::Tz(_) => Time {
                        at: day.at + 86_399 - self.offset.clamp(0, 14 * 60) * 60,
                        zone: Zone::Utc,
                    },
                })
            }
            _ => None,
        };
        let mut by_day = Vec::new();
        for d in &self.days {
            let wd = DAYS
                .iter()
                .position(|x| x.eq_ignore_ascii_case(d))
                .ok_or("a repeat day is not a day of the week")?;
            by_day.push((0, wd as i64));
        }
        if self.freq != Freq::Weekly {
            by_day.clear();
        }
        Ok(Rule {
            freq: self.freq,
            interval: self.interval,
            count: self.count.filter(|c| (1..=10_000).contains(c)),
            until,
            by_day,
            by_month_day: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
        })
    }
}

/// An event as the page sends it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Fields {
    pub summary: String,
    pub location: String,
    pub description: String,
    /// `2026-10-22` (all day) or `2026-10-22T08:00`.
    pub start: String,
    pub end: String,
    /// The zone of both times; none for all-day events.
    pub zone: Option<String>,
    pub repeat: Option<Repeat>,
    pub organizer: Option<Person>,
    pub attendees: Vec<Person>,
    /// Minutes before the start.
    pub alarms: Vec<i64>,
    /// `video`, `voice` or empty.
    pub call: String,
    /// Without `repeat`: keep the repeat the event has, which the page
    /// could not show (a rule of its own, a phone's RDATEs).
    pub keep_repeat: bool,
}

impl Fields {
    fn times(&self) -> Result<(Time, Time), String> {
        let all_day = !self.start.contains('T');
        let zone = if all_day { None } else { self.zone.as_deref() };
        let start = Time::from_page(&self.start, zone).ok_or("the start is not a date or time")?;
        let end = Time::from_page(&self.end, zone).ok_or("the end is not a date or time")?;
        if start.is_date() != end.is_date() {
            return Err("the start and end are not both days or both times".into());
        }
        if end.at < start.at || (start.is_date() && end.at == start.at) {
            return Err("the event ends before it starts".into());
        }
        Ok((start, end))
    }
}

/// The lines of a VEVENT the page owns: [`rewrite`] replaces these and
/// keeps every other line a client wrote.
const OWNED: &[&str] = &[
    "UID",
    "DTSTAMP",
    "SUMMARY",
    "LOCATION",
    "DESCRIPTION",
    "DTSTART",
    "DTEND",
    "DURATION",
    "RRULE",
    "EXDATE",
    "ORGANIZER",
    "ATTENDEE",
    "X-LOSOS-CALL",
    "SEQUENCE",
    "LAST-MODIFIED",
];

/// UTC now as an iCalendar DATE-TIME.
fn stamp(now: i64) -> String {
    Time {
        at: now,
        zone: Zone::Utc,
    }
    .property("X")
    .rsplit(':')
    .next()
    .unwrap_or("")
    .to_owned()
}

/// The page's own lines of a VEVENT, alarms included.
fn owned_lines(
    uid: &str,
    f: &Fields,
    sequence: i64,
    now: i64,
    old: &[Person],
    exdates: &[String],
) -> Result<Vec<String>, String> {
    let (start, end) = f.times()?;
    let mut out = vec![
        format!("UID:{uid}"),
        format!("DTSTAMP:{}", stamp(now)),
        format!("LAST-MODIFIED:{}", stamp(now)),
        format!("SEQUENCE:{sequence}"),
        start.property("DTSTART"),
        end.property("DTEND"),
    ];
    let summary = bounded(&f.summary, MAX_TEXT);
    if summary.is_empty() {
        return Err("the event needs a title".into());
    }
    out.push(format!("SUMMARY:{}", escape(&summary)));
    for (name, v) in [("LOCATION", &f.location), ("DESCRIPTION", &f.description)] {
        let v = bounded(v, MAX_TEXT);
        if !v.is_empty() {
            out.push(format!("{name}:{}", escape(&v)));
        }
    }
    if let Some(r) = &f.repeat {
        out.push(format!("RRULE:{}", r.rule(&start)?.text()));
        out.extend(exdates.iter().cloned());
    }
    let clean = |p: &Person| -> Result<Person, String> {
        let email = p.email.trim().to_ascii_lowercase();
        if email.len() > 254
            || email.split('@').count() != 2
            || email
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "<>\",;:".contains(c))
        {
            return Err(format!("{} is not an email address", p.email));
        }
        Ok(Person {
            email,
            name: bounded(&p.name, 200),
            status: String::new(),
            matrix: bounded(&p.matrix, 255),
        })
    };
    if !f.attendees.is_empty() {
        let organizer = f
            .organizer
            .as_ref()
            .ok_or("an event with guests needs an organizer")?;
        let organizer = clean(organizer)?;
        out.push(organizer.line("ORGANIZER", false));
        let mut seen = std::collections::HashSet::new();
        for p in f.attendees.iter().take(MAX_PEOPLE) {
            let mut p = clean(p)?;
            if !seen.insert(p.email.clone()) {
                continue;
            }
            // Someone who already answered keeps their answer; the
            // organizer is there as having said yes.
            p.status = old
                .iter()
                .find(|o| o.email == p.email)
                .map(|o| o.status.clone())
                .unwrap_or_default();
            if p.email == organizer.email {
                p.status = "ACCEPTED".into();
            }
            out.push(p.line("ATTENDEE", true));
        }
    }
    match f.call.as_str() {
        "video" | "voice" => out.push(format!("X-LOSOS-CALL:{}", f.call)),
        "" => {}
        _ => return Err("a call is video or voice".into()),
    }
    let mut alarms: Vec<i64> = f
        .alarms
        .iter()
        .copied()
        .filter(|m| (0..=40_320).contains(m))
        .collect();
    alarms.sort_unstable();
    alarms.dedup();
    for m in alarms.into_iter().take(MAX_ALARMS) {
        out.push("BEGIN:VALARM".into());
        out.push("ACTION:DISPLAY".into());
        out.push(format!("DESCRIPTION:{}", escape(&summary)));
        out.push(format!("TRIGGER:-PT{m}M"));
        out.push("END:VALARM".into());
    }
    Ok(out)
}

/// A new calendar object holding one event from the page.
pub fn create(uid: &str, f: &Fields, now: i64) -> Result<String, String> {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".to_owned(),
        "BEGIN:VEVENT".to_owned(),
    ];
    lines.extend(owned_lines(uid, f, 0, now, &[], &[])?);
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());
    Ok(join(&lines))
}

/// The same object with its main event's own lines replaced by `f`'s, and
/// everything else a client wrote kept: other properties, its time zones,
/// and changed instances of a repeat (dropped when it no longer repeats).
pub fn rewrite(old: &str, f: &Fields, now: i64) -> Result<String, String> {
    let lines = unfold(old);
    let (head, comps) = components(&lines);
    let main = comps
        .iter()
        .position(|(n, l)| {
            n == "VEVENT" && !l.iter().any(|x| Line::parse(x).name == "RECURRENCE-ID")
        })
        .ok_or("the object has no event")?;
    let prev = event(&comps[main].1).ok_or("the event has no start")?;
    let mut out = vec!["BEGIN:VCALENDAR".to_owned()];
    out.extend(head.into_iter().filter(|l| Line::parse(l).name != "METHOD"));
    for (i, (name, comp)) in comps.iter().enumerate() {
        if i != main {
            let instance = comp.iter().any(|x| Line::parse(x).name == "RECURRENCE-ID");
            if !(instance && f.repeat.is_none() && !f.keep_repeat) {
                out.extend(comp.iter().cloned());
            }
            let _ = name;
            continue;
        }
        // The EXDATEs stay with the repeat while it starts at the same time.
        let same_start = f.times().map(|(s, _)| s == prev.start).unwrap_or(false);
        let exdates: Vec<String> = if same_start {
            comp.iter()
                .filter(|l| Line::parse(l).name == "EXDATE")
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let mut kept = Vec::new();
        let mut depth = 0usize;
        let mut in_alarm = false;
        for l in comp {
            let p = Line::parse(l);
            match p.name.as_str() {
                "BEGIN" => {
                    depth += 1;
                    if depth == 2 && p.value.trim().eq_ignore_ascii_case("VALARM") {
                        in_alarm = true;
                    }
                    if depth == 1 || in_alarm {
                        continue;
                    }
                }
                "END" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        continue;
                    }
                    if in_alarm && depth == 1 {
                        in_alarm = false;
                        continue;
                    }
                }
                _ => {}
            }
            let kept_repeat = f.repeat.is_none()
                && f.keep_repeat
                && matches!(p.name.as_str(), "RRULE" | "EXDATE");
            if in_alarm || (depth == 1 && OWNED.contains(&p.name.as_str()) && !kept_repeat) {
                continue;
            }
            kept.push(l.clone());
        }
        out.push("BEGIN:VEVENT".into());
        out.extend(owned_lines(
            &prev.uid,
            f,
            prev.sequence + 1,
            now,
            &prev.attendees,
            &exdates,
        )?);
        out.extend(kept);
        out.push("END:VEVENT".into());
    }
    out.push("END:VCALENDAR".into());
    Ok(join(&out))
}

/// The object with one occurrence of its repeat left out (`EXDATE`).
pub fn exclude(old: &str, at: &Time) -> Result<String, String> {
    let lines = unfold(old);
    let mut out = Vec::with_capacity(lines.len() + 1);
    let mut depth = 0usize;
    let mut main = false;
    let mut done = false;
    for l in &lines {
        let p = Line::parse(l);
        match p.name.as_str() {
            "BEGIN" => {
                depth += 1;
                if depth == 2 {
                    main = p.value.trim().eq_ignore_ascii_case("VEVENT");
                }
            }
            "RECURRENCE-ID" if depth == 2 => main = false,
            "END" => {
                if depth == 2 && main && !done {
                    out.push(at.property("EXDATE"));
                    done = true;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        out.push(l.clone());
    }
    if !done {
        return Err("the object has no event".into());
    }
    Ok(join(&out))
}

// ── Files of many events ───────────────────────────────────────────────────

/// One calendar object cut out of a file: its UID and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub uid: String,
    pub text: String,
}

/// Cut a file into one calendar object per UID: every VEVENT and VTODO of
/// that UID, with the VTIMEZONEs they name. A component without a UID gets
/// one made from its content, so reading the same file twice makes the
/// same objects. `METHOD` is dropped (a stored object has none).
pub fn split(text: &str, limit: usize) -> Vec<Piece> {
    let lines = unfold(text);
    if !lines
        .first()
        .is_some_and(|l| l.trim().eq_ignore_ascii_case("BEGIN:VCALENDAR"))
    {
        return Vec::new();
    }
    let (_, comps) = components(&lines);
    let zones: Vec<(String, &Vec<String>)> = comps
        .iter()
        .filter(|(n, _)| n == "VTIMEZONE")
        .filter_map(|(_, l)| {
            l.iter()
                .map(|x| Line::parse(x))
                .find(|x| x.name == "TZID")
                .map(|x| (x.value.trim().to_owned(), l))
        })
        .collect();
    let mut order: Vec<String> = Vec::new();
    let mut by_uid: std::collections::HashMap<String, Vec<&Vec<String>>> = Default::default();
    for (name, comp) in &comps {
        if name != "VEVENT" && name != "VTODO" {
            continue;
        }
        let uid = comp
            .iter()
            .map(|x| Line::parse(x))
            .find(|x| x.name == "UID")
            .map(|x| bounded(&x.value, 255))
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "{}@losos",
                    &crate::cloud::util::sha1_hex(comp.join("\n").as_bytes())[..24]
                )
            });
        if !by_uid.contains_key(&uid) {
            if order.len() >= limit {
                continue;
            }
            order.push(uid.clone());
        }
        by_uid.entry(uid).or_default().push(comp);
    }
    order
        .into_iter()
        .map(|uid| {
            let comps = &by_uid[&uid];
            let mut used: Vec<&str> = Vec::new();
            for comp in comps {
                for l in comp.iter() {
                    let p = Line::parse(l);
                    if let Some(z) = p.param("TZID") {
                        if let Some((name, _)) = zones.iter().find(|(n, _)| n == z) {
                            if !used.contains(&name.as_str()) {
                                used.push(name);
                            }
                        }
                    }
                }
            }
            let mut out = vec![
                "BEGIN:VCALENDAR".to_owned(),
                "VERSION:2.0".to_owned(),
                format!("PRODID:{PRODID}"),
            ];
            for (name, z) in &zones {
                if used.contains(&name.as_str()) {
                    out.extend(z.iter().cloned());
                }
            }
            let has_uid = comps
                .iter()
                .any(|c| c.iter().any(|x| Line::parse(x).name == "UID"));
            for comp in comps {
                if has_uid {
                    out.extend(comp.iter().cloned());
                } else {
                    // The made-up UID goes in right after BEGIN.
                    out.push(comp[0].clone());
                    out.push(format!("UID:{uid}"));
                    out.extend(comp[1..].iter().cloned());
                }
            }
            out.push("END:VCALENDAR".into());
            Piece {
                uid,
                text: join(&out),
            }
        })
        .collect()
}

/// A whole calendar as one file: every object's components under one
/// VCALENDAR, each VTIMEZONE once. What a published feed serves.
pub fn bundle(name: &str, color: &str, objects: &[String]) -> String {
    let mut out = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".to_owned(),
        "METHOD:PUBLISH".to_owned(),
        format!("X-WR-CALNAME:{}", escape(name)),
        format!("NAME:{}", escape(name)),
        "REFRESH-INTERVAL;VALUE=DURATION:PT1H".to_owned(),
        "X-PUBLISHED-TTL:PT1H".to_owned(),
    ];
    if !color.is_empty() {
        out.push(format!("COLOR:{}", color.trim_start_matches('#')));
        out.push(format!("X-APPLE-CALENDAR-COLOR:{color}"));
    }
    let mut zones_seen = std::collections::HashSet::new();
    let mut body = Vec::new();
    for o in objects {
        let lines = unfold(o);
        let (_, comps) = components(&lines);
        for (n, comp) in comps {
            if n == "VTIMEZONE" {
                let id = comp
                    .iter()
                    .map(|x| Line::parse(x))
                    .find(|x| x.name == "TZID")
                    .map(|x| x.value);
                if id.is_some_and(|id| zones_seen.insert(id)) {
                    out.extend(comp);
                }
            } else {
                // The owner's reminders are theirs, not the subscribers'.
                body.extend(without_alarms(&comp));
            }
        }
    }
    out.extend(body);
    out.push("END:VCALENDAR".into());
    join(&out)
}

fn without_alarms(comp: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(comp.len());
    let mut depth = 0usize;
    let mut skip = false;
    for l in comp {
        let p = Line::parse(l);
        match p.name.as_str() {
            "BEGIN" => {
                depth += 1;
                if depth == 2 && p.value.trim().eq_ignore_ascii_case("VALARM") {
                    skip = true;
                }
            }
            "END" => {
                let was = skip;
                if depth == 2 {
                    skip = false;
                }
                depth = depth.saturating_sub(1);
                if was {
                    continue;
                }
            }
            _ => {}
        }
        if !skip {
            out.push(l.clone());
        }
    }
    out
}

// ── Invitations (iTIP) ─────────────────────────────────────────────────────

/// An invitation to send the guests: the object's events under
/// `METHOD:REQUEST`, without the organizer's reminders.
pub fn request(object: &str) -> String {
    let lines = unfold(object);
    let (head, comps) = components(&lines);
    let mut out = vec!["BEGIN:VCALENDAR".to_owned()];
    out.extend(head.into_iter().filter(|l| Line::parse(l).name != "METHOD"));
    out.push("METHOD:REQUEST".into());
    for (_, comp) in comps {
        out.extend(without_alarms(&comp));
    }
    out.push("END:VCALENDAR".into());
    join(&out)
}

/// The same text with `email`'s PARTSTAT set to `status` in every event of
/// it, and `METHOD` dropped. `None` when `email` is not an attendee.
pub fn with_partstat(text: &str, email: &str, status: &str) -> Option<String> {
    let email = email.trim().to_ascii_lowercase();
    let mut found = false;
    let mut out = Vec::new();
    for l in unfold(text) {
        let mut p = Line::parse(&l);
        if p.name == "METHOD" {
            continue;
        }
        if p.name == "ATTENDEE" && Person::from_line(&p).is_some_and(|who| who.email == email) {
            found = true;
            p.set_param("PARTSTAT", status);
            p.params.retain(|(k, _)| k != "RSVP");
            out.push(p.text());
            continue;
        }
        out.push(l);
    }
    found.then(|| join(&out))
}

/// The answer to an invitation from `email`: `METHOD:REPLY` with only the
/// lines RFC 5546 asks a reply to carry.
pub fn reply(invite: &str, email: &str, status: &str, now: i64) -> Option<String> {
    let email = email.trim().to_ascii_lowercase();
    let lines = unfold(invite);
    let (_, comps) = components(&lines);
    let mut out = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        format!("PRODID:{PRODID}"),
        "METHOD:REPLY".to_owned(),
    ];
    let mut any = false;
    for (name, comp) in comps {
        if name != "VEVENT" {
            continue;
        }
        let mut me = None;
        let mut keep = Vec::new();
        let mut depth = 0usize;
        for l in &comp {
            let p = Line::parse(l);
            match p.name.as_str() {
                "BEGIN" => depth += 1,
                "END" => depth = depth.saturating_sub(1),
                _ if depth == 1 => match p.name.as_str() {
                    "UID" | "RECURRENCE-ID" | "SEQUENCE" | "ORGANIZER" | "DTSTART" | "DTEND"
                    | "DURATION" | "SUMMARY" => keep.push(l.clone()),
                    "ATTENDEE" if Person::from_line(&p).is_some_and(|w| w.email == email) => {
                        let mut p = p.clone();
                        p.set_param("PARTSTAT", status);
                        p.params.retain(|(k, _)| k != "RSVP");
                        me = Some(p.text());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let Some(me) = me else { continue };
        any = true;
        out.push("BEGIN:VEVENT".into());
        out.push(format!("DTSTAMP:{}", stamp(now)));
        out.extend(keep);
        out.push(me);
        out.push("END:VEVENT".into());
    }
    out.push("END:VCALENDAR".into());
    any.then(|| join(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i64, m: i64, d: i64) -> i64 {
        days_from_civil(y, m, d) * 86_400
    }

    fn at(y: i64, m: i64, d: i64, h: i64, mi: i64) -> i64 {
        day(y, m, d) + h * 3600 + mi * 60
    }

    fn local(t: i64) -> String {
        Time {
            at: t,
            zone: Zone::Floating,
        }
        .local()
    }

    fn one(text: &str) -> Event {
        let cal = parse(text);
        assert_eq!(cal.events.len(), 1, "{text}");
        cal.events.into_iter().next().unwrap()
    }

    fn vevent(lines: &[&str]) -> String {
        let mut v = vec!["BEGIN:VCALENDAR", "VERSION:2.0", "BEGIN:VEVENT"];
        v.extend_from_slice(lines);
        v.extend_from_slice(&["END:VEVENT", "END:VCALENDAR", ""]);
        v.join("\r\n")
    }

    #[test]
    fn civil_days_round_trip_and_weekdays() {
        for d in [-1000, 0, 365, 19_000, 20_745, 60_000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        // 2026-10-22 is a Thursday.
        assert_eq!(weekday(days_from_civil(2026, 10, 22)), 3);
        assert_eq!(weekday(days_from_civil(2024, 2, 29)), 3);
    }

    #[test]
    fn times_keep_how_they_were_meant() {
        let p = vec![("TZID".to_owned(), "Europe/Bratislava".to_owned())];
        let t = parse_time("20261022T080000", &p).unwrap();
        assert_eq!(t.zone, Zone::Tz("Europe/Bratislava".into()));
        assert_eq!(t.local(), "2026-10-22T08:00:00");
        assert_eq!(parse_time("20261022T060000Z", &p).unwrap().zone, Zone::Utc);
        let d = parse_time("20260331", &[]).unwrap();
        assert!(d.is_date());
        assert_eq!(d.local(), "2026-03-31");
        assert_eq!(parse_time("20260231", &[]), None);
        // Outlook's names and Mozilla's paths become IANA names.
        let outlook = vec![("TZID".to_owned(), "Central Europe Standard Time".to_owned())];
        assert_eq!(
            parse_time("20261022T080000", &outlook).unwrap().zone,
            Zone::Tz("Europe/Budapest".into())
        );
        let moz = vec![(
            "TZID".to_owned(),
            "/mozilla.org/20050126_1/Europe/Vienna".to_owned(),
        )];
        assert_eq!(
            parse_time("20261022T080000", &moz).unwrap().zone,
            Zone::Tz("Europe/Vienna".into())
        );
        assert_eq!(
            Time::from_page("2026-10-22T08:30", Some("Europe/Bratislava"))
                .unwrap()
                .property("DTSTART"),
            "DTSTART;TZID=Europe/Bratislava:20261022T083000"
        );
        assert_eq!(Time::from_page("2026-02-30", None), None);
        assert_eq!(Time::from_page("2026-10-22T08:30", Some("a;b")), None);
    }

    #[test]
    fn lines_with_quoted_parameters_and_escapes_read_right() {
        let l =
            Line::parse("ATTENDEE;CN=\"Nováková, Eva\";PARTSTAT=ACCEPTED:mailto:eva@mattbox.local");
        assert_eq!(l.name, "ATTENDEE");
        assert_eq!(l.param("CN"), Some("Nováková, Eva"));
        assert_eq!(l.value, "mailto:eva@mattbox.local");
        assert_eq!(
            l.text(),
            "ATTENDEE;CN=\"Nováková, Eva\";PARTSTAT=ACCEPTED:mailto:eva@mattbox.local"
        );
        assert_eq!(unescape("a\\, b\\; c\\nd\\\\"), "a, b; c\nd\\");
        assert_eq!(escape("a, b; c\nd\\"), "a\\, b\\; c\\nd\\\\");
        let long = format!("SUMMARY:{}", "č".repeat(60));
        let folded = fold(&long);
        assert!(folded.split("\r\n").all(|l| l.len() <= 75));
        assert_eq!(unfold(&folded), vec![long]);
    }

    #[test]
    fn an_event_reads_with_its_people_reminders_and_call() {
        let e = one(&vevent(&[
            "UID:e1",
            "DTSTART;TZID=Europe/Bratislava:20261022T100000",
            "DTEND;TZID=Europe/Bratislava:20261022T113000",
            "SUMMARY:Budget review\\, Q4",
            "LOCATION:Kitchen",
            "ORGANIZER;CN=Matus:mailto:matus@mattbox.local",
            "ATTENDEE;CN=Eva;PARTSTAT=TENTATIVE:mailto:Eva@mattbox.local",
            "X-LOSOS-CALL:video",
            "BEGIN:VALARM",
            "ACTION:DISPLAY",
            "TRIGGER:-PT15M",
            "END:VALARM",
            "BEGIN:VALARM",
            "TRIGGER;RELATED=START:-P1D",
            "END:VALARM",
        ]));
        assert_eq!(e.summary, "Budget review, Q4");
        assert_eq!(e.length, 5400);
        assert_eq!(e.organizer.as_ref().unwrap().name, "Matus");
        assert_eq!(e.attendees[0].email, "eva@mattbox.local");
        assert_eq!(e.attendees[0].status, "TENTATIVE");
        assert_eq!(e.alarms, vec![900, 86_400]);
        assert_eq!(e.call, "video");
        // A day with no end lasts the day.
        let d = one(&vevent(&[
            "UID:d",
            "DTSTART;VALUE=DATE:20260331",
            "SUMMARY:Tax",
        ]));
        assert_eq!(d.length, 86_400);
        assert!(d.start.is_date());
    }

    #[test]
    fn a_weekly_lesson_repeats_on_its_days_and_skips_its_exdates() {
        let e = one(&vevent(&[
            "UID:t1",
            "DTSTART;TZID=Europe/Bratislava:20260907T080000",
            "DTEND;TZID=Europe/Bratislava:20260907T084500",
            "RRULE:FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20261216T230000Z",
            "EXDATE;TZID=Europe/Bratislava:20261028T080000",
            "SUMMARY:Spanish",
        ]));
        let list = starts(&e, day(2026, 10, 19), day(2026, 11, 2), &[], 100);
        let shown: Vec<String> = list.iter().map(|t| local(*t)).collect();
        assert_eq!(
            shown,
            vec![
                "2026-10-19T08:00:00",
                "2026-10-21T08:00:00",
                "2026-10-26T08:00:00"
            ]
        );
        // The last one is the Wednesday the UNTIL names; nothing after.
        let end = starts(&e, day(2026, 12, 1), day(2027, 2, 1), &[], 100);
        assert_eq!(local(*end.last().unwrap()), "2026-12-16T08:00:00");
        // An overridden instance is left to its own event.
        let skip = starts(
            &e,
            day(2026, 10, 19),
            day(2026, 10, 20),
            &[at(2026, 10, 19, 8, 0)],
            10,
        );
        assert!(skip.is_empty());
    }

    #[test]
    fn yearly_and_monthly_deadlines_land_on_their_days() {
        // The tax return, every 31 March, all day.
        let tax = one(&vevent(&[
            "UID:tax",
            "DTSTART;VALUE=DATE:20260331",
            "RRULE:FREQ=YEARLY",
            "SUMMARY:Income tax return",
        ]));
        let list = starts(&tax, day(2026, 1, 1), day(2029, 1, 1), &[], 10);
        assert_eq!(
            list,
            vec![day(2026, 3, 31), day(2027, 3, 31), day(2028, 3, 31)]
        );
        // VAT, the 25th of every month, three times.
        let vat = one(&vevent(&[
            "UID:vat",
            "DTSTART;VALUE=DATE:20261025",
            "RRULE:FREQ=MONTHLY;BYMONTHDAY=25;COUNT=3",
            "SUMMARY:VAT return",
        ]));
        let list = starts(&vat, day(2026, 1, 1), day(2028, 1, 1), &[], 10);
        assert_eq!(
            list,
            vec![day(2026, 10, 25), day(2026, 11, 25), day(2026, 12, 25)]
        );
        // The last working day of each month.
        let last = one(&vevent(&[
            "UID:last",
            "DTSTART:20261030T090000",
            "RRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1",
            "SUMMARY:Payroll",
        ]));
        let list = starts(&last, day(2026, 10, 1), day(2027, 2, 1), &[], 10);
        let shown: Vec<String> = list.iter().map(|t| local(*t)).collect();
        assert_eq!(
            shown,
            vec![
                "2026-10-30T09:00:00",
                "2026-11-30T09:00:00",
                "2026-12-31T09:00:00",
                "2027-01-29T09:00:00"
            ]
        );
        // The second Tuesday, and 29 February only in leap years.
        let second = one(&vevent(&[
            "UID:s",
            "DTSTART:20261013T180000",
            "RRULE:FREQ=MONTHLY;BYDAY=2TU",
        ]));
        let list = starts(&second, day(2026, 11, 1), day(2026, 12, 1), &[], 10);
        assert_eq!(list, vec![at(2026, 11, 10, 18, 0)]);
        let leap = one(&vevent(&[
            "UID:l",
            "DTSTART;VALUE=DATE:20240229",
            "RRULE:FREQ=YEARLY",
        ]));
        let list = starts(&leap, day(2025, 1, 1), day(2029, 1, 1), &[], 10);
        assert_eq!(list, vec![day(2028, 2, 29)]);
    }

    #[test]
    fn a_daily_repeat_from_long_ago_starts_near_the_window() {
        let e = one(&vevent(&[
            "UID:old",
            "DTSTART:20000103T070000Z",
            "RRULE:FREQ=DAILY;INTERVAL=2",
        ]));
        let list = starts(&e, day(2026, 10, 20), day(2026, 10, 26), &[], 10);
        assert!(!list.is_empty() && list.len() <= 3, "{list:?}");
        // Every other day from 2000-01-03 lands on even day gaps.
        for t in &list {
            assert_eq!((t / 86_400 - days_from_civil(2000, 1, 3)) % 2, 0);
        }
        // A count is walked from the start.
        let counted = one(&vevent(&[
            "UID:c",
            "DTSTART:20000103T070000Z",
            "RRULE:FREQ=DAILY;COUNT=5",
        ]));
        assert!(starts(&counted, day(2026, 1, 1), day(2027, 1, 1), &[], 10).is_empty());
        assert_eq!(
            starts(&counted, day(2000, 1, 1), day(2000, 2, 1), &[], 10).len(),
            5
        );
    }

    fn fields() -> Fields {
        Fields {
            summary: "Budget review".into(),
            location: "Kitchen, upstairs".into(),
            start: "2026-10-23T10:00".into(),
            end: "2026-10-23T11:00".into(),
            zone: Some("Europe/Bratislava".into()),
            organizer: Some(Person {
                email: "matus@mattbox.local".into(),
                name: "Matus".into(),
                ..Default::default()
            }),
            attendees: vec![Person {
                email: "Eva@mattbox.local".into(),
                name: "Eva".into(),
                matrix: "@eva:mattbox.local".into(),
                ..Default::default()
            }],
            alarms: vec![15, 15, 60],
            call: "video".into(),
            ..Default::default()
        }
    }

    #[test]
    fn an_event_the_page_writes_reads_back_the_same() {
        let text = create("uid-1@losos", &fields(), at(2026, 10, 22, 7, 0)).unwrap();
        assert!(
            text.contains("DTSTART;TZID=Europe/Bratislava:20261023T100000\r\n"),
            "{text}"
        );
        assert!(text.contains("LOCATION:Kitchen\\, upstairs\r\n"), "{text}");
        assert!(text.contains("TRIGGER:-PT15M\r\n") && text.contains("TRIGGER:-PT60M\r\n"));
        assert_eq!(text.matches("BEGIN:VALARM").count(), 2);
        let e = one(&text);
        assert_eq!(e.summary, "Budget review");
        assert_eq!(e.location, "Kitchen, upstairs");
        assert_eq!(e.length, 3600);
        assert_eq!(e.alarms, vec![900, 3600]);
        assert_eq!(e.attendees.len(), 1);
        assert_eq!(e.attendees[0].email, "eva@mattbox.local");
        assert_eq!(e.attendees[0].status, "NEEDS-ACTION");
        assert_eq!(e.attendees[0].matrix, "@eva:mattbox.local");
        assert_eq!(e.call, "video");
        // Refused: no title, an end before the start, a bad address.
        let mut bad = fields();
        bad.summary = " ".into();
        assert!(create("u", &bad, 0).is_err());
        let mut bad = fields();
        bad.end = "2026-10-23T09:00".into();
        assert!(create("u", &bad, 0).is_err());
        let mut bad = fields();
        bad.attendees[0].email = "eva@x\r\nATTENDEE:mailto:x@y".into();
        assert!(create("u", &bad, 0).is_err());
    }

    #[test]
    fn an_edit_keeps_what_a_phone_wrote_and_who_answered() {
        let phone = vevent(&[
            "UID:p1",
            "DTSTAMP:20261001T000000Z",
            "DTSTART;TZID=Europe/Bratislava:20261023T100000",
            "DTEND;TZID=Europe/Bratislava:20261023T110000",
            "SUMMARY:Old title",
            "ORGANIZER:mailto:matus@mattbox.local",
            "ATTENDEE;PARTSTAT=ACCEPTED:mailto:eva@mattbox.local",
            "X-APPLE-TRAVEL-ADVISORY-BEHAVIOR:AUTOMATIC",
            "CATEGORIES:Work",
            "SEQUENCE:2",
            "BEGIN:VALARM",
            "TRIGGER:-PT5M",
            "END:VALARM",
        ]);
        let out = rewrite(&phone, &fields(), at(2026, 10, 22, 7, 0)).unwrap();
        let e = one(&out);
        assert_eq!(e.summary, "Budget review");
        assert_eq!(e.uid, "p1");
        assert_eq!(e.sequence, 3);
        assert_eq!(e.attendees[0].status, "ACCEPTED");
        assert_eq!(e.alarms, vec![900, 3600]);
        assert!(out.contains("X-APPLE-TRAVEL-ADVISORY-BEHAVIOR:AUTOMATIC\r\n"));
        assert!(out.contains("CATEGORIES:Work\r\n"));
        assert!(!out.contains("Old title") && !out.contains("-PT5M"));
        assert_eq!(out.matches("BEGIN:VEVENT").count(), 1);
    }

    #[test]
    fn a_repeat_the_page_cannot_show_stays_when_asked() {
        let phone = vevent(&[
            "UID:p2",
            "DTSTART;TZID=Europe/Bratislava:20261023T100000",
            "DTEND;TZID=Europe/Bratislava:20261023T110000",
            "SUMMARY:Choir",
            "RRULE:FREQ=MONTHLY;BYDAY=2TU,4TU",
            "EXDATE;TZID=Europe/Bratislava:20261110T100000",
        ]);
        let mut f = fields();
        f.attendees.clear();
        f.keep_repeat = true;
        let out = rewrite(&phone, &f, 0).unwrap();
        assert!(
            out.contains("RRULE:FREQ=MONTHLY;BYDAY=2TU,4TU\r\n"),
            "{out}"
        );
        assert!(out.contains("EXDATE;TZID=Europe/Bratislava:20261110T100000\r\n"));
        f.keep_repeat = false;
        let out = rewrite(&phone, &f, 0).unwrap();
        assert!(!out.contains("RRULE") && !out.contains("EXDATE"));
    }

    #[test]
    fn a_repeat_from_the_page_and_one_occurrence_left_out() {
        let mut f = fields();
        f.attendees.clear();
        f.repeat = Some(Repeat {
            freq: Freq::Weekly,
            interval: 1,
            until: Some("2026-11-30".into()),
            count: None,
            days: vec!["FR".into(), "MO".into()],
            offset: 60,
        });
        let text = create("r1", &f, 0).unwrap();
        assert!(
            text.contains("RRULE:FREQ=WEEKLY;UNTIL=20261130T225959Z;BYDAY=FR,MO\r\n"),
            "{text}"
        );
        let e = one(&text);
        assert_eq!(
            Repeat::of(e.rule.as_ref().unwrap()).unwrap().days,
            vec!["FR", "MO"]
        );
        let all = starts(&e, day(2026, 11, 23), day(2026, 12, 8), &[], 10);
        assert_eq!(
            all.iter().map(|t| local(*t)).collect::<Vec<_>>(),
            vec![
                "2026-11-23T10:00:00",
                "2026-11-27T10:00:00",
                "2026-11-30T10:00:00"
            ]
        );
        let out = exclude(
            &text,
            &Time {
                at: at(2026, 11, 27, 10, 0),
                zone: Zone::Tz("Europe/Bratislava".into()),
            },
        )
        .unwrap();
        let e = one(&out);
        assert_eq!(
            starts(&e, day(2026, 11, 23), day(2026, 12, 8), &[], 10).len(),
            2
        );
        // The EXDATE stays through an edit that keeps the start.
        let again = rewrite(&out, &f, 0).unwrap();
        assert!(
            again.contains("EXDATE;TZID=Europe/Bratislava:20261127T100000\r\n"),
            "{again}"
        );
    }

    const FEED: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Feed//EN\r\nMETHOD:PUBLISH\r\nX-WR-CALNAME:Tax deadlines\r\nX-PUBLISHED-TTL:PT12H\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Bratislava\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/London\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:a@feed\r\nDTSTART;VALUE=DATE:20270331\r\nSUMMARY:Income tax return\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b@feed\r\nDTSTART;TZID=Europe/Bratislava:20261025T090000\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:VAT\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b@feed\r\nRECURRENCE-ID;TZID=Europe/Bratislava:20261125T090000\r\nDTSTART;TZID=Europe/Bratislava:20261126T090000\r\nSUMMARY:VAT (moved)\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART:20261101T100000Z\r\nSUMMARY:No UID\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn a_feed_splits_into_objects_with_their_zones() {
        let cal = parse(FEED);
        assert_eq!(cal.name, "Tax deadlines");
        assert_eq!(cal.refresh, Some(12 * 3600));
        assert_eq!(cal.method, "PUBLISH");
        let pieces = split(FEED, 100);
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].uid, "a@feed");
        assert!(!pieces[0].text.contains("VTIMEZONE"));
        assert!(pieces[1].text.contains("TZID:Europe/Bratislava"));
        assert!(!pieces[1].text.contains("Europe/London"));
        assert_eq!(pieces[1].text.matches("BEGIN:VEVENT").count(), 2);
        assert!(!pieces.iter().any(|p| p.text.contains("METHOD")));
        // The made-up UID is the same every time.
        assert!(pieces[2].uid.ends_with("@losos"));
        assert_eq!(split(FEED, 100)[2], pieces[2]);
        assert!(pieces[2].text.contains(&format!("UID:{}", pieces[2].uid)));
        assert_eq!(split(FEED, 2).len(), 2);
        assert!(split("not a calendar", 10).is_empty());
        // And back as one feed, each zone once, no reminders.
        let all = bundle(
            "Mine",
            "#0a7d5a",
            &pieces.iter().map(|p| p.text.clone()).collect::<Vec<_>>(),
        );
        let again = parse(&all);
        assert_eq!(again.events.len(), 4);
        assert_eq!(again.name, "Mine");
        assert_eq!(all.matches("TZID:Europe/Bratislava").count(), 1);
    }

    #[test]
    fn an_invitation_is_answered_and_the_answer_recorded() {
        let mine = create("inv-1", &fields(), 0).unwrap();
        let req = request(&mine);
        assert!(req.contains("METHOD:REQUEST\r\n"));
        assert!(!req.contains("VALARM"));
        let cal = parse(&req);
        assert_eq!(cal.method, "REQUEST");
        // Eva says yes: her copy, and the reply to Matus.
        let hers = with_partstat(&req, "EVA@mattbox.local", "ACCEPTED").unwrap();
        assert!(!hers.contains("METHOD"));
        assert_eq!(one(&hers).attendees[0].status, "ACCEPTED");
        assert!(with_partstat(&req, "nobody@mattbox.local", "ACCEPTED").is_none());
        let answer = reply(&req, "eva@mattbox.local", "ACCEPTED", 0).unwrap();
        let r = parse(&answer);
        assert_eq!(r.method, "REPLY");
        assert_eq!(r.events[0].uid, "inv-1");
        assert_eq!(r.events[0].attendees.len(), 1);
        assert!(!answer.contains("RSVP"));
        assert!(reply(&req, "nobody@mattbox.local", "ACCEPTED", 0).is_none());
        // Matus records it on his own copy.
        let updated = with_partstat(&mine, "eva@mattbox.local", "ACCEPTED").unwrap();
        assert_eq!(one(&updated).attendees[0].status, "ACCEPTED");
        assert!(updated.contains("BEGIN:VALARM"));
    }
}
