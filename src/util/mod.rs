//! 시간 포맷 등 작은 유틸.

use chrono::{DateTime, Local, Utc};

pub fn format_local_dt(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_local_time(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M").to_string()
}

pub fn format_duration_human(seconds: i64) -> String {
    let s = seconds.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}시간 {m}분")
    } else if m > 0 {
        format!("{m}분 {sec}초")
    } else {
        format!("{sec}초")
    }
}
