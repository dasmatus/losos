//! `<Date>` written out as words, so a page shows it with no script: in the
//! document's language (its front matter's `lang`), else the reader's,
//! else English. The `datetime` attribute keeps the ISO date beside it.

/// The languages LosOS is written in; any other falls back to English.
pub const LANGS: &[&str] = &["en", "sk", "de"];

const EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

// Slovak dates take the month in the genitive.
const SK: [&str; 12] = [
    "januára",
    "februára",
    "marca",
    "apríla",
    "mája",
    "júna",
    "júla",
    "augusta",
    "septembra",
    "októbra",
    "novembra",
    "decembra",
];

const DE: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// `en`, `sk` or `de` for a language tag such as `de-AT`, else `en`.
pub fn lang(tag: &str) -> &'static str {
    let primary = tag
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    LANGS
        .iter()
        .find(|l| **l == primary)
        .copied()
        .unwrap_or("en")
}

fn days_in(year: u32, month: u32) -> u32 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `value` (YYYY-MM-DD) in words; `long` spells the month out. A date that
/// does not exist is written as it was given.
pub fn format(value: &str, long: bool, lang_tag: &str) -> String {
    let parts: Vec<&str> = value.split('-').collect();
    let [y, m, d] = parts[..] else {
        return value.to_string();
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return value.to_string();
    };
    if y.len() != 4 || !(1..=12).contains(&month) || day == 0 || day > days_in(year, month) {
        return value.to_string();
    }
    let i = month as usize - 1;
    match (lang(lang_tag), long) {
        ("sk", false) => format!("{day}. {month}. {year}"),
        ("sk", true) => format!("{day}. {} {year}", SK[i]),
        ("de", false) => format!("{day:02}.{month:02}.{year}"),
        ("de", true) => format!("{day}. {} {year}", DE[i]),
        (_, false) => format!("{day} {} {year}", &EN[i][..3]),
        (_, true) => format!("{day} {} {year}", EN[i]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_each_language() {
        assert_eq!(format("2026-08-14", false, "en"), "14 Aug 2026");
        assert_eq!(format("2026-08-14", true, "en-GB"), "14 August 2026");
        assert_eq!(format("2026-08-04", false, "sk"), "4. 8. 2026");
        assert_eq!(format("2026-10-28", true, "sk"), "28. októbra 2026");
        assert_eq!(format("2026-03-04", false, "de-AT"), "04.03.2026");
        assert_eq!(format("2026-03-04", true, "de"), "4. März 2026");
        assert_eq!(format("2026-03-04", true, "fr"), "4 March 2026");
    }

    #[test]
    fn a_date_that_does_not_exist_stays_as_written() {
        assert_eq!(format("2026-02-29", true, "en"), "2026-02-29");
        assert_eq!(format("2024-02-29", true, "en"), "29 February 2024");
        assert_eq!(format("2026-13-01", true, "en"), "2026-13-01");
        assert_eq!(format("2026-00-10", true, "en"), "2026-00-10");
        assert_eq!(format("26-01-01", true, "en"), "26-01-01");
    }
}
