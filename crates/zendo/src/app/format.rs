//! Numbers and times as people read them.

/// "1 game", "12 games".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "45 min", "3 h", "18 h 10 min".
pub fn play_time(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// How long before `now` a time was: "today", "3 days ago".
pub fn ago(then: u64, now: u64) -> String {
    let days = now.saturating_sub(then) / 86_400;
    match days {
        0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        2..=13 => format!("{days} days ago"),
        14..=59 => format!("{} weeks ago", days / 7),
        60..=729 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_like_people_write() {
        assert_eq!(count(1, "game", "games"), "1 game");
        assert_eq!(count(0, "game", "games"), "0 games");
        assert_eq!(play_time(45), "45 min");
        assert_eq!(play_time(180), "3 h");
        assert_eq!(play_time(1090), "18 h 10 min");
        let day = 86_400;
        assert_eq!(ago(10 * day, 10 * day + 5), "today");
        assert_eq!(ago(0, day), "yesterday");
        assert_eq!(ago(0, 20 * day), "2 weeks ago");
        assert_eq!(ago(0, 90 * day), "3 months ago");
        assert_eq!(ago(0, 800 * day), "2 years ago");
    }
}
