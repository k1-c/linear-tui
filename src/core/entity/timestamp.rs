//! Timestamps in the one shape snapshots use: RFC 3339, UTC, whole seconds
//! (`2026-09-25T06:01:02Z`). A fixed shape sorts as text in time order and
//! needs no date library to write or read back. Reading the clock is the
//! caller's: these only convert.

pub fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Read back a timestamp written by [`format_timestamp`].
pub fn parse_timestamp(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<u64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let days = u64::try_from(days_from_civil(y as i64, m as i64, d as i64)).ok()?;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// How long ago `text` was, in words: `just now`, `5 minutes ago`.
pub fn ago(text: &str, now: u64) -> Option<String> {
    let secs = now.saturating_sub(parse_timestamp(text)?);
    let (n, unit) = match secs {
        0..60 => return Some("just now".into()),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    let plural = if n == 1 { "" } else { "s" };
    Some(format!("{n} {unit}{plural} ago"))
}

// Howard Hinnant's algorithms for the proleptic Gregorian calendar.

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A timestamp is RFC 3339 in UTC, to the second, across leap years
    /// and the century.
    #[test]
    fn timestamps_are_rfc3339_utc_to_the_second() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_timestamp(1_790_316_062), "2026-09-25T06:01:02Z");
        assert_eq!(format_timestamp(4_102_444_799), "2099-12-31T23:59:59Z");
    }
}
