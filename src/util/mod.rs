//! ============================================================================
//! util — 시간 포맷 등 공통 헬퍼.
//! ============================================================================
//!
//! UTC `DateTime` 을 **회사 timezone**(`PolicySnapshot.time_zone_offset_minutes`,
//! 기본 540=KST) 기준으로 변환해 화면에 표시한다.
//!
//! ── 정책 (2026-05-14) ─────────────────────────────────────────────────
//! 사용자 PC OS 의 timezone 과 무관하게, **회사 정책의 timezone 기준** 으로
//! 모든 시간을 표시한다. 회사 지사가 해외에 있는 케이스 대응 + 사용자 PC 의
//! timezone 이 잘못 설정된 환경에서도 일관된 표시 보장.
//!
//! `format_local_*` 함수들은 직전 정책(PC OS Local) 의 잔재이며, 신규 코드는
//! `format_company_*` 사용 권장.

use chrono::{DateTime, FixedOffset, Local, Utc};

/// 회사 timezone offset(분, `-720~840`) → `FixedOffset` 변환.
/// 범위 밖이면 KST(+540) 으로 fallback.
pub fn company_offset(minutes: i32) -> FixedOffset {
    let secs = minutes.clamp(-720, 840) as i32 * 60;
    FixedOffset::east_opt(secs).unwrap_or_else(|| FixedOffset::east_opt(540 * 60).unwrap())
}

/// "YYYY-MM-DD HH:MM:SS" 형식 (회사 timezone 기준). 자세한 시각 표시용.
pub fn format_company_dt(dt: &DateTime<Utc>, offset_minutes: i32) -> String {
    dt.with_timezone(&company_offset(offset_minutes))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// "HH:MM" 형식 (회사 timezone 기준). 같은 날 안의 자리비움 시작/종료 표시용.
pub fn format_company_time(dt: &DateTime<Utc>, offset_minutes: i32) -> String {
    dt.with_timezone(&company_offset(offset_minutes))
        .format("%H:%M")
        .to_string()
}

/// (deprecated) PC OS Local 기준 자세한 시각 — 회사 timezone 정책 도입 전 잔재.
/// 신규 코드는 `format_company_dt` 사용. 점진 교체용 보존.
pub fn format_local_dt(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

/// (deprecated) PC OS Local 기준 "HH:MM" — `format_company_time` 으로 교체 권장.
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
