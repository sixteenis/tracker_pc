//! ============================================================================
//! util — 시간 포맷 등 공통 헬퍼.
//! ============================================================================
//!
//! UTC `DateTime` 을 사용자 로컬 타임존으로 변환해 화면에 표시할 때 사용.
//! 모든 DB 저장값은 UTC; UI 표시 시점에만 변환한다.

use chrono::{DateTime, Local, Utc};

/// "YYYY-MM-DD HH:MM:SS" 형식 (로컬 타임존). 자세한 시각 표시용.
pub fn format_local_dt(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

/// "HH:MM" 형식 (로컬 타임존). 같은 날 안의 자리비움 시작/종료 표시용.
pub fn format_local_time(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M").to_string()
}

/// 초 단위 → "2시간 5분" / "5분 12초" / "8초" 한글 표기.
/// 음수는 0 으로 클램프.
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
