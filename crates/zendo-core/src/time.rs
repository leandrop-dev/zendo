//! Unix seconds: now, from the ISO-8601 dates Heroic writes, and the year
//! of one.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// `2026-11-06T01:08:40.436Z` (UTC, fraction optional) as Unix seconds.
/// Anything else, including an empty string, is `None`.
pub fn parse_utc(text: &str) -> Option<u64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let time = time.split_once('.').map_or(time, |(whole, _)| whole);
    let mut time = time.splitn(3, ':').map(str::parse::<i64>);
    let (hour, minute, second) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    // A launcher's date is outside Zendo's control: a year it cannot mean
    // would overflow the day count.
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }
    // A day the month does not have is not quietly rolled into the next
    // month.
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days).contains(&day) {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    u64::try_from(seconds).ok()
}

/// The year (UTC) of a moment in Unix seconds.
pub fn year_of(seconds: i64) -> i64 {
    civil_from_days(seconds.div_euclid(86_400)).0
}

/// A moment in Unix seconds as `2026-11-07 14:05 UTC`.
pub fn utc(seconds: u64) -> String {
    let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let minutes = seconds.rem_euclid(86_400) / 60;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        minutes / 60,
        minutes % 60
    )
}

/// The weekday of a day counted from 1970-01-01 (a Thursday), Monday first:
/// 0 is Monday and 6 is Sunday.
pub(crate) fn weekday_of(days: i64) -> usize {
    usize::try_from((days + 3).rem_euclid(7)).unwrap_or(0)
}

/// The date of a day counted from 1970-01-01, in the proleptic Gregorian
/// calendar (Howard Hinnant's `civil_from_days`).
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    (era * 400 + year_of_era + i64::from(month <= 2), month, day)
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar (Howard
/// Hinnant's `days_from_civil`).
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_heroic_dates() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2000-03-01T00:00:00.000Z"), Some(951_868_800));
        assert_eq!(parse_utc("2026-11-06T01:08:40.436Z"), Some(1_793_927_320));
        assert_eq!(parse_utc(""), None);
        assert_eq!(parse_utc("2026-13-01T00:00:00Z"), None);
        // A year no date can hold is refused, not overflowed.
        assert_eq!(parse_utc("10000000000-01-01T00:00:00Z"), None);
        assert_eq!(parse_utc("-2026-01-01T00:00:00Z"), None);
        // A day the month does not have is refused, not rolled over.
        assert_eq!(parse_utc("2026-02-31T00:00:00Z"), None);
        assert_eq!(parse_utc("2026-04-31T00:00:00Z"), None);
        assert_eq!(parse_utc("2023-02-29T00:00:00Z"), None);
        assert!(parse_utc("2024-02-29T00:00:00Z").is_some());
    }

    #[test]
    fn gives_the_year() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(951_868_800), 2000);
        assert_eq!(year_of(1_793_927_320), 2026);
        // The first and last second of 1999.
        assert_eq!(year_of(915_148_800), 1999);
        assert_eq!(year_of(946_684_799), 1999);
        assert_eq!(year_of(-1), 1969);
    }

    #[test]
    fn gives_the_weekday_monday_first() {
        // 1970-01-01 was a Thursday, 2026-11-13 a Friday, 2000-01-01 a Saturday.
        assert_eq!(weekday_of(0), 3);
        assert_eq!(weekday_of(days_from_civil(2026, 11, 13)), 4);
        assert_eq!(weekday_of(days_from_civil(2000, 1, 1)), 5);
        assert_eq!(weekday_of(-1), 2, "the Wednesday before the epoch");
        assert_eq!(weekday_of(days_from_civil(2026, 11, 15)), 6);
    }

    #[test]
    fn writes_dates_for_people() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(951_868_800), "2000-03-01 00:00 UTC");
        assert_eq!(utc(1_793_927_320), "2026-11-06 01:08 UTC");
        assert_eq!(utc(946_684_799), "1999-12-31 23:59 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00 UTC");
    }
}
