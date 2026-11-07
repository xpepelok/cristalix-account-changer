use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn parse_rfc3339(text: &str) -> Option<SystemTime> {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = digits(&bytes[0..4])?;
    let month = digits(&bytes[5..7])?;
    let day = digits(&bytes[8..10])?;
    let hour = digits(&bytes[11..13])?;
    let minute = digits(&bytes[14..16])?;
    let second = digits(&bytes[17..19])?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let mut cursor = 19;
    let mut nanos = 0u32;
    if bytes[cursor] == b'.' {
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            if cursor - start < 9 {
                nanos = nanos * 10 + u32::from(bytes[cursor] - b'0');
            }
            cursor += 1;
        }
        let taken = cursor - start;
        if taken == 0 {
            return None;
        }
        nanos *= 10u32.pow(9 - taken.min(9) as u32);
    }

    let offset = match bytes.get(cursor)? {
        b'Z' | b'z' if cursor + 1 == bytes.len() => 0,
        sign @ (b'+' | b'-') if cursor + 6 == bytes.len() && bytes[cursor + 3] == b':' => {
            let hours = digits(&bytes[cursor + 1..cursor + 3])?;
            let minutes = digits(&bytes[cursor + 4..cursor + 6])?;
            let total = hours * 3600 + minutes * 60;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };

    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    if seconds >= 0 {
        Some(UNIX_EPOCH + Duration::new(seconds as u64, nanos))
    } else {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs(seconds.unsigned_abs()))
            .map(|base| base + Duration::from_nanos(u64::from(nanos)))
    }
}

fn digits(bytes: &[u8]) -> Option<i64> {
    bytes.iter().try_fold(0i64, |acc, byte| {
        byte.is_ascii_digit()
            .then(|| acc * 10 + i64::from(byte - b'0'))
    })
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::parse_rfc3339;
    use std::time::{Duration, UNIX_EPOCH};

    fn at(seconds: u64, nanos: u32) -> Option<std::time::SystemTime> {
        Some(UNIX_EPOCH + Duration::new(seconds, nanos))
    }

    #[test]
    fn parses_utc_with_fraction() {
        assert_eq!(
            parse_rfc3339("2026-11-05T00:09:14.659529231Z"),
            at(1_793_837_354, 659_529_231)
        );
        assert_eq!(
            parse_rfc3339("2023-02-01T19:47:22.168Z"),
            at(1_675_280_842, 168_000_000)
        );
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), at(0, 0));
    }

    #[test]
    fn applies_offsets() {
        assert_eq!(
            parse_rfc3339("2026-11-05T03:09:14.5+03:00"),
            at(1_793_837_354, 500_000_000)
        );
        assert_eq!(
            parse_rfc3339("2026-11-04T23:09:14-01:00"),
            at(1_793_837_354, 0)
        );
    }

    #[test]
    fn rejects_garbage() {
        for text in [
            "",
            "2026-11-05",
            "2026-13-05T00:00:00Z",
            "2026-11-05T00:00:00",
            "2026-11-05T00:00:00.Z",
            "2026-11-05T00:00:00+0300",
        ] {
            assert_eq!(parse_rfc3339(text), None, "{text}");
        }
    }
}
