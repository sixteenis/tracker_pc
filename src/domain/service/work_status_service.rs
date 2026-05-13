//! ============================================================================
//! domain::service::work_status_service — 근로자 출퇴근 판별 (2026-05-12 신규).
//! ============================================================================
//!
//! 단일 진실 소스: V1 `/android/u/get_workstatus.jsp` 응답 + `main_info` 의 starttm/endtm.
//!
//! ── 판별 규칙 (사용자 결정 2026-05-12) ────────────────────────────
//!   - `result > 0`                                                    → **WorkingNow** (근무중)
//!   - `result == 0` AND starttm 비거나 "00:00"
//!                  AND endtm   비거나 "00:00"                          → **NotIn** (미출근)
//!   - `result == 0` AND (starttm 또는 endtm 값이 있고 "00:00" 아님)   → **OffWork** (퇴근)
//!
//! UI 출근 카드 라벨, idle_detector 의 추적 게이트가 본 결과를 사용.
//!
//! ── 캐시 ──────────────────────────────────────────────────────────
//! `sync::user_info_sync` 가 `get_work_status` 호출 후 `set_result` 로 갱신.
//! UI / 엔진은 `current_status()` 로 동기 조회.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::domain::model::main_info::MainInfo;

/// 판별된 출퇴근 상태 — UI 라벨, 엔진 게이트가 동일하게 사용.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkStatus {
    /// 아직 응답 없음 (로그인 직후 짧은 시간 또는 네트워크 끊김).
    Unknown,
    /// 출근 전 — starttm/endtm 둘 다 비어있거나 "00:00".
    NotIn,
    /// 근무중 — `result > 0`.
    WorkingNow,
    /// 퇴근 — `result == 0` 이지만 starttm/endtm 중 하나라도 값 있음.
    OffWork,
}

impl WorkStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "—",
            Self::NotIn => "미출근",
            Self::WorkingNow => "근무중",
            Self::OffWork => "퇴근",
        }
    }
    /// idle_detector 가 segment 를 만들어야 하는 상태인지.
    /// 근무중일 때만 true (Unknown 은 안전상 false — 정보가 부족할 때 segment 생성 안 함).
    pub fn allows_tracking(self) -> bool {
        matches!(self, Self::WorkingNow)
    }
}

static CURRENT_RESULT: Lazy<RwLock<Option<i64>>> = Lazy::new(|| RwLock::new(None));

/// `sync::user_info_sync` 가 매 폴링마다 호출.
pub fn set_result(result: i64) {
    if let Ok(mut g) = CURRENT_RESULT.write() {
        *g = Some(result);
    }
}

/// 로그아웃 시 호출 — 캐시 초기화.
pub fn clear() {
    if let Ok(mut g) = CURRENT_RESULT.write() {
        *g = None;
    }
}

/// 현재 result 캐시 (없으면 None).
pub fn current_result() -> Option<i64> {
    CURRENT_RESULT.read().ok().and_then(|g| *g)
}

/// `result` + `MainInfo` 조합으로 출퇴근 상태 판별.
/// 응답 없으면 `Unknown` (UI 가 "—" 표시).
pub fn current_status() -> WorkStatus {
    let result = current_result();
    let main_info = crate::domain::service::main_info_service::current();
    derive(result, main_info.as_ref())
}

/// 순수 함수 — 테스트 가능.
pub fn derive(result: Option<i64>, main_info: Option<&MainInfo>) -> WorkStatus {
    let Some(r) = result else {
        return WorkStatus::Unknown;
    };
    if r > 0 {
        return WorkStatus::WorkingNow;
    }
    // result == 0
    let start_blank = main_info.map(|m| is_blank_or_zero(&m.start_time)).unwrap_or(true);
    let end_blank = main_info.map(|m| is_blank_or_zero(&m.end_time)).unwrap_or(true);
    if start_blank && end_blank {
        WorkStatus::NotIn
    } else {
        WorkStatus::OffWork
    }
}

/// 빈 문자열 / 공백 / "00:00" 은 미설정으로 본다.
fn is_blank_or_zero(s: &str) -> bool {
    let t = s.trim();
    t.is_empty() || t == "00:00"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::main_info::MainInfo;

    fn mk(start: &str, end: &str) -> MainInfo {
        MainInfo {
            start_time: start.to_string(),
            end_time: end.to_string(),
            join_date: String::new(),
            remaining_annual_minutes: 0,
            work_minutes: 0,
            add_minutes: 0,
            used_minutes: 0,
            unread_message_count: 0,
            deduct_on_late: false,
            deduct_on_early_leave: false,
            deduct_on_outing: false,
            annual_by_join_date: false,
            daily_annual_by_join_date: false,
            use_break_time: false,
            use_schedule: false,
            auto_checkout_mode: 0,
            commute_notify: false,
            work_52h_unit: 0,
        }
    }

    #[test]
    fn unknown_when_no_result() {
        assert_eq!(derive(None, None), WorkStatus::Unknown);
    }

    #[test]
    fn working_when_positive_result() {
        assert_eq!(derive(Some(14148240), None), WorkStatus::WorkingNow);
        assert_eq!(derive(Some(1), Some(&mk("", ""))), WorkStatus::WorkingNow);
    }

    #[test]
    fn not_in_when_zero_and_blank() {
        assert_eq!(derive(Some(0), None), WorkStatus::NotIn);
        assert_eq!(derive(Some(0), Some(&mk("", ""))), WorkStatus::NotIn);
        assert_eq!(derive(Some(0), Some(&mk("00:00", "00:00"))), WorkStatus::NotIn);
        assert_eq!(derive(Some(0), Some(&mk("   ", "  "))), WorkStatus::NotIn);
    }

    #[test]
    fn off_work_when_zero_and_filled() {
        assert_eq!(derive(Some(0), Some(&mk("09:00", "18:00"))), WorkStatus::OffWork);
        assert_eq!(derive(Some(0), Some(&mk("09:00", ""))), WorkStatus::OffWork);
        assert_eq!(derive(Some(0), Some(&mk("", "18:00"))), WorkStatus::OffWork);
    }
}
