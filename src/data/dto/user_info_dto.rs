//! ============================================================================
//! data::dto::user_info_dto — `GET /api/pc-agent/user-info` 응답 DTO.
//! ============================================================================
//!
//! 로그인 직후 1회 + 그 이후 적응형 주기로 호출.
//! - `attendance_status == "WORKING"` → `next_poll_seconds = 3600` (1시간)
//! - 그 외 → `next_poll_seconds = 300` (5분)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::AttendanceStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoSnapshot {
    pub user: UserInfoUser,
    pub subscription: UserInfoSubscription,
    pub attendance: UserInfoAttendance,

    pub polled_at: DateTime<Utc>,
    pub next_poll_seconds: u64,

    #[serde(default)]
    pub force_logout: bool,

    /// 회사 소명사유 목록의 `MAX(UPD_DT)` epoch (Phase 1.b, 2026-05-12).
    /// 클라가 캐시값과 비교해 다르면 `GET /api/pc-agent/explanation-types` 재호출.
    /// 서버가 응답에 누락하면 0 (재호출 트리거 안 됨).
    #[serde(default)]
    pub explanation_types_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoUser {
    pub employee_id: String,
    #[serde(default)]
    pub employee_name: String,
    #[serde(default)]
    pub english_name: String,
    pub company_id: String,
    #[serde(default)]
    pub company_name: String,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub team_name: String,
    #[serde(default)]
    pub team_template_id: Option<String>,
    #[serde(default)]
    pub team_template_name: String,
    #[serde(default)]
    pub position: String,
    #[serde(default)]
    pub employee_number: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub authority: i32,
    #[serde(default)]
    pub join_date: Option<String>,
    #[serde(default)]
    pub leave_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoSubscription {
    pub plan_code: String,
    pub payment_status: String,
    pub pc_tracking_enabled: bool,
    pub can_track_time: bool,
    #[serde(default)]
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoAttendance {
    pub attendance_status: AttendanceStatus,
    #[serde(default)]
    pub work_start_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub work_end_at: Option<DateTime<Utc>>,
}
