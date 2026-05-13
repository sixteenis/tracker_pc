//! ============================================================================
//! domain::model::main_info — 로그인 후 메인 정보 도메인 모델.
//! ============================================================================
//!
//! `data::dto::main_info_dto::MainInfoResponseDto` 의 wire 약어를 의미 있는
//! 이름으로 변환한 결과. 모바일 클라이언트 응답에는 비콘/위치/광고 등 PC
//! Agent 와 무관한 항목이 많지만, 본 모델은 PC 근무시간 추적·표시에 필요한
//! 항목만 노출한다.

#[derive(Debug, Clone)]
pub struct MainInfo {
    /// 출근 시각 (서버 `starttm`, "HH:MM").
    pub start_time: String,
    /// 퇴근 시각 (서버 `endtm`, "HH:MM").
    pub end_time: String,
    /// 입사일 (서버 `joindt`). "1900-01-01" 이면 미설정.
    pub join_date: String,

    /// 남은 연차 (분). 서버 `anual`.
    pub remaining_annual_minutes: i64,
    /// 오늘 근로 합계 (분). 서버 `workmin`.
    pub work_minutes: i64,
    /// 추가 근로 (분). 서버 `addmin`.
    pub add_minutes: i64,
    /// 사용 근로 (분). 서버 `usemin`.
    pub used_minutes: i64,

    /// 미확인 메시지 수.
    pub unread_message_count: i32,

    /// 지각 시 연차 차감 여부 (서버 `anualddctn1`).
    pub deduct_on_late: bool,
    /// 조퇴 시 연차 차감 여부 (서버 `anualddctn2`).
    pub deduct_on_early_leave: bool,
    /// 외출 시 연차 차감 여부 (서버 `anualddctn3`).
    pub deduct_on_outing: bool,

    /// 입사일 기준(true) / 회계년도(false) — 서버 `stAnual`.
    pub annual_by_join_date: bool,
    /// 일 단위 연차 입사일 기준 — 서버 `stDAnual`.
    pub daily_annual_by_join_date: bool,

    /// 휴게시간 사용 (서버 `brkTime`).
    pub use_break_time: bool,
    /// 근무일정 사용 (서버 `schdl`).
    pub use_schedule: bool,
    /// 자동 퇴근기록 모드 — 서버 `cmtLt`.
    /// 1: 출근시간 기준, 2: 회사퇴근시간 기준, 3: 사용 안 함.
    pub auto_checkout_mode: i32,
    /// 출퇴근 전 알림 (서버 `cmtnoti`).
    pub commute_notify: bool,
    /// 주52시간 단위 — 0: 주, 1: 월 (서버 `wk52h`).
    pub work_52h_unit: i32,
}

impl MainInfo {
    /// 입사일이 "1900-01-01" 인지 — 입사일 미설정 안내가 필요한 케이스.
    pub fn is_join_date_unset(&self) -> bool {
        self.join_date.is_empty() || self.join_date.starts_with("1900-01-01")
    }
}
