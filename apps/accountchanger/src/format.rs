use std::fmt::Write as _;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Utc};

pub fn now_secs() -> i64 {
    Utc::now().timestamp()
}

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn relative_time(ts: i64, now: i64) -> Option<String> {
    if ts == 0 {
        return None;
    }
    let diff = now - ts;
    Some(match diff {
        d if d < 60 => "только что".to_owned(),
        d if d < 3600 => format!("{} мин назад", d / 60),
        d if d < 86400 => format!("{} ч назад", d / 3600),
        d => format!("{} дн назад", d / 86400),
    })
}

const SPAN_UNITS: [(&str, i64); 6] = [
    ("Г", 31_536_000),
    ("М", 2_592_000),
    ("д", 86_400),
    ("ч", 3_600),
    ("м", 60),
    ("с", 1),
];

pub fn span(millis: i64) -> String {
    let total = (millis / 1000).max(0);
    let index = SPAN_UNITS
        .iter()
        .position(|(_, size)| total >= *size)
        .unwrap_or(SPAN_UNITS.len() - 1);
    let (unit, size) = SPAN_UNITS[index];
    let main = total / size;
    let mut out = format!("{main}{unit}");
    if let Some((next_unit, next_size)) = SPAN_UNITS.get(index + 1) {
        let rest = (total - main * size) / next_size;
        if rest > 0 {
            write!(out, " {rest}{next_unit}").ok();
        }
    }
    out
}

fn local(ts: i64) -> Option<DateTime<Local>> {
    Local.timestamp_opt(ts, 0).single()
}

pub fn date_time(ts: i64) -> String {
    match local(ts).filter(|_| ts != 0) {
        Some(at) => format!(
            "{:02}.{:02}.{} {:02}:{:02}",
            at.day(),
            at.month(),
            at.year(),
            at.hour(),
            at.minute()
        ),
        None => "-".to_owned(),
    }
}

pub fn parse_iso_millis(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|at| at.timestamp_millis())
}

pub fn iso_date_time(iso: &str) -> String {
    match parse_iso_millis(iso).and_then(|ms| local(ms.div_euclid(1000))) {
        Some(at) => format!(
            "{:02}.{:02}.{} {:02}:{:02}:{:02}",
            at.day(),
            at.month(),
            at.year(),
            at.hour(),
            at.minute(),
            at.second()
        ),
        None => "-".to_owned(),
    }
}

pub fn duration_words(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h} ч {m:02} мин")
    } else if m > 0 {
        format!("{m} мин {s:02} с")
    } else {
        format!("{s} с")
    }
}

pub fn clock_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn session_time(ts: i64) -> String {
    match local(ts).filter(|_| ts != 0) {
        Some(at) => format!(
            "{:02}:{:02}:{:02} {:02}-{:02}-{}",
            at.hour(),
            at.minute(),
            at.second(),
            at.day(),
            at.month(),
            at.year()
        ),
        None => "—".to_owned(),
    }
}

pub fn session_day(ts: i64) -> (i32, u32, u32) {
    local(ts).map_or((0, 0, 0), |at| (at.year(), at.month(), at.day()))
}

pub fn session_label(started: i64, ordinal: usize) -> String {
    let (year, month, day) = session_day(started);
    format!("{day:02}-{month:02}-{year} ({ordinal})")
}

pub fn count(n: u64) -> String {
    let short = |value: f64, unit: &str| {
        if value.fract() == 0.0 {
            format!("{value}{unit}")
        } else {
            format!("{value:.1}{unit}")
        }
    };
    match n {
        n if n >= 1_000_000 => short(n as f64 / 1_000_000.0, "kk"),
        n if n >= 1_000 => short(n as f64 / 1_000.0, "k"),
        n => n.to_string(),
    }
}

pub fn bytes(n: u64) -> String {
    match n {
        n if n < 1024 => format!("{n} Б"),
        n if n < 1024 * 1024 => format!("{:.0} КБ", n as f64 / 1024.0),
        n => format!("{:.1} МБ", n as f64 / (1024.0 * 1024.0)),
    }
}

pub fn megabytes(n: u64) -> String {
    format!("{:.2}", n as f64 / 1_048_576.0)
}

pub fn subscription_label(key: &str) -> String {
    key.strip_prefix("level_")
        .filter(|level| !level.is_empty() && level.bytes().all(|b| b.is_ascii_digit()))
        .map_or_else(|| key.to_owned(), |level| format!("Ур. {level}"))
}

pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_matches_js() {
        assert_eq!(relative_time(0, 100), None);
        assert_eq!(relative_time(100, 130).unwrap(), "только что");
        assert_eq!(relative_time(1, 2701).unwrap(), "45 мин назад");
        assert_eq!(relative_time(1, 7202).unwrap(), "2 ч назад");
        assert_eq!(relative_time(1, 172_802).unwrap(), "2 дн назад");
    }

    #[test]
    fn spans_show_two_units() {
        assert_eq!(span(5_000), "5с");
        assert_eq!(span(65_000), "1м 5с");
        assert_eq!(span(3_600_000), "1ч");
        assert_eq!(span(90_061_000), "1д 1ч");
        assert_eq!(span(-5), "0с");
    }

    #[test]
    fn durations_format() {
        assert_eq!(duration_words(42), "42 с");
        assert_eq!(duration_words(125), "2 мин 05 с");
        assert_eq!(duration_words(3720), "1 ч 02 мин");
        assert_eq!(clock_duration(65), "01:05");
        assert_eq!(clock_duration(3665), "01:01:05");
    }

    #[test]
    fn counts_and_bytes() {
        assert_eq!(count(999), "999");
        assert_eq!(count(1000), "1k");
        assert_eq!(count(1500), "1.5k");
        assert_eq!(count(2_000_000), "2kk");
        assert_eq!(bytes(512), "512 Б");
        assert_eq!(bytes(2048), "2 КБ");
        assert_eq!(bytes(3 * 1024 * 1024), "3.0 МБ");
    }

    #[test]
    fn misc_labels() {
        assert_eq!(subscription_label("level_3"), "Ур. 3");
        assert_eq!(subscription_label("gold"), "gold");
        assert_eq!(file_name("C:\\a\\b\\launcher.exe"), "launcher.exe");
        assert_eq!(file_name("/opt/x.jar"), "x.jar");
        assert_eq!(date_time(0), "-");
        assert!(parse_iso_millis("2024-05-01T10:00:00Z").is_some());
    }
}
