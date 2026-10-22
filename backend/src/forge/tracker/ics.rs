//! Due dates as a calendar: an all-day event per issue, RFC 5545, so a
//! calendar (LosOS Cloud's, a phone's) shows what is due when.

use super::store::Ticket;
use super::Category;

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
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

/// Lines longer than 75 octets go on in a line that starts with a space.
fn fold(line: &str, out: &mut String) {
    let mut width = 0;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if width + n > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(ch);
        width += n;
    }
    out.push_str("\r\n");
}

fn day(d: &str) -> String {
    d.replace('-', "")
}

fn next_day(d: &str) -> String {
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map(|d| (d + chrono::Duration::days(1)).format("%Y%m%d").to_string())
        .unwrap_or_else(|_| day(d))
}

/// One calendar of the issues that have a due date. `host` makes the
/// event ids unique to the box; `base` is where the pages are.
pub fn calendar(tickets: &[Ticket], name: &str, host: &str, base: &str) -> String {
    let mut out = String::new();
    for l in [
        "BEGIN:VCALENDAR",
        "VERSION:2.0",
        "PRODID:-//LosOS//LosOS Git issues//EN",
        "CALSCALE:GREGORIAN",
    ] {
        fold(l, &mut out);
    }
    fold(&format!("X-WR-CALNAME:{}", escape(name)), &mut out);
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    for t in tickets {
        let Some(due) = t.due.as_deref() else {
            continue;
        };
        fold("BEGIN:VEVENT", &mut out);
        fold(
            &format!("UID:{}@{}", t.key.to_ascii_lowercase(), escape(host)),
            &mut out,
        );
        fold(&format!("DTSTAMP:{stamp}"), &mut out);
        fold(&format!("DTSTART;VALUE=DATE:{}", day(due)), &mut out);
        fold(&format!("DTEND;VALUE=DATE:{}", next_day(due)), &mut out);
        fold(
            &format!("SUMMARY:{}", escape(&format!("{} {}", t.key, t.title))),
            &mut out,
        );
        let url = format!("https://{host}{base}/issues/{}", t.key);
        fold(&format!("URL:{}", escape(&url)), &mut out);
        fold(
            &format!(
                "DESCRIPTION:{}",
                escape(&format!("{}\n\n{url}", t.state.name))
            ),
            &mut out,
        );
        if t.state.category == Category::Canceled {
            fold("STATUS:CANCELLED", &mut out);
        }
        fold("END:VEVENT", &mut out);
    }
    fold("END:VCALENDAR", &mut out);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn folding_and_escaping() {
        let mut out = String::new();
        super::fold(&"x".repeat(80), &mut out);
        assert_eq!(out, format!("{}\r\n {}\r\n", "x".repeat(75), "x".repeat(5)));
        assert_eq!(super::escape("a, b; c\nd"), "a\\, b\\; c\\nd");
        assert_eq!(super::next_day("2026-12-31"), "20270101");
    }
}
