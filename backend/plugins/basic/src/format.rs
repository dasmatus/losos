//! Numbers as the script pages print them (screens/settings/format.ts and
//! lib/apps.ts), with the reader's decimal mark.

use losos_page::i18n::{Locale, Messages};

fn decimal(locale: Locale, value: f64, places: usize) -> String {
    let s = format!("{value:.places$}");
    match locale {
        Locale::En => s,
        Locale::Sk | Locale::De => s.replace('.', ","),
    }
}

/// screens/settings/format.ts's formatBytes: powers of 1024, one decimal
/// under ten.
#[must_use]
pub fn bytes_1024(locale: Locale, bytes: Option<u64>) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let Some(bytes) = bytes else {
        return "—".to_string();
    };
    if bytes < 1024 {
        return format!(
            "{} B",
            locale.number(i64::try_from(bytes).unwrap_or(i64::MAX))
        );
    }
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let places = if value >= 10.0 { 0 } else { 1 };
    format!("{} {}", decimal(locale, value, places), UNITS[unit])
}

/// lib/apps.ts's formatBytes: powers of 1000, one decimal under ten.
#[must_use]
pub fn bytes_1000(m: &Messages, bytes: u64) -> String {
    const UNITS: [&str; 6] = ["bytes", "kB", "MB", "GB", "TB", "PB"];
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        return m.tv("apps.format.bytes", &[("count", bytes.into())]);
    }
    let places = if value < 10.0 { 1 } else { 0 };
    format!("{} {}", decimal(m.locale, value, places), UNITS[unit])
}

/// A share of a whole as a whole percent, for the `w-[N%]` classes.
#[must_use]
pub fn percent(part: u64, whole: u64) -> u64 {
    if whole == 0 {
        return 0;
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let p = ((part as f64 / whole as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u64;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_read_like_the_script_pages() {
        assert_eq!(bytes_1024(Locale::En, Some(512)), "512 B");
        assert_eq!(bytes_1024(Locale::En, Some(1536)), "1.5 KB");
        assert_eq!(
            bytes_1024(Locale::De, Some(5 * 1024 * 1024 * 1024 / 2)),
            "2,5 GB"
        );
        assert_eq!(
            bytes_1024(Locale::En, Some(40 * 1024 * 1024 * 1024)),
            "40 GB"
        );
        assert_eq!(bytes_1024(Locale::En, None), "—");
        let m = Messages::from_pairs(Locale::En, &[]);
        assert_eq!(bytes_1000(&m, 2_500_000_000), "2.5 GB");
        assert_eq!(bytes_1000(&m, 25_000_000_000), "25 GB");
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(5, 0), 0);
    }
}
