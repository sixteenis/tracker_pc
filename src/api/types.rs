//! ============================================================================
//! api::types — 서버 ↔ PC 앱 데이터 모델 (모든 DTO).
//! ============================================================================
//!
//! 모든 시간값은 RFC3339 (UTC) 문자열. 시간만 (HH:MM) 으로 표현되는 점심
//! 정책은 `String` 으로 받아 로컬에서 파싱한다 (`lunch::parse_hhmm`).
//!
//! `serde(rename_all = "SCREAMING_SNAKE_CASE")` 로 enum 들이 서버 JSON 의
//! 대문자 코드 (예: "WORKING", "BUSINESS_TRIP") 와 매핑된다.
//!
//! TODO(서버 연동): 핀플 백엔드 응답 스키마와 1:1 비교 검증 필요. 누락된 필드가
//! 있으면 `#[serde(default)]` 추가하거나 `Option<T>` 로 완화.
//! TODO(2차): API 버전 헤더 추가 (`X-Api-Version: 1`) — 서버가 호환성 정보를
//! 응답에 실어주면 클라이언트가 자동 업그레이드 안내 가능.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 출근 상태 — PC 앱이 직접 변경하지 않음. 서버 조회 결과만 저장.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttendanceStatus {
    Working,
    BeforeWork,
    AfterWork,
    Outing,
    Leave,
    BusinessTrip,
    Unknown,
}

impl Default for AttendanceStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl AttendanceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Working => "출근 중",
            Self::BeforeWork => "출근 전",
            Self::AfterWork => "퇴근 후",
            Self::Outing => "외출 중",
            Self::Leave => "연차",
            Self::BusinessTrip => "출장",
            Self::Unknown => "알 수 없음",
        }
    }
    pub fn enables_tracking(&self) -> bool {
        matches!(self, Self::Working)
    }
}

// ───────────────────────── 1. Login / Refresh ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub login_id: String,
    pub password: String,
    pub device_id: String,
    pub device_name: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub device_id: String,
    pub device_name: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_in: i64, // seconds

    pub company_id: String,
    pub employee_id: String,
    pub employee_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,

    pub subscription: SubscriptionInfo,
    pub policy: PolicySnapshot,

    /// 다른 PC 에서 활성 로그인 중이던 device_id (있다면).
    /// 클라이언트는 단순 안내만; 서버가 이미 강제 로그아웃을 처리한다.
    pub displaced_device: Option<DisplacedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub plan_code: String,
    pub payment_status: String,    // ACTIVE / SUSPENDED / EXPIRED
    pub pc_tracking_enabled: bool, // 회사 단위 PC 감지 ON/OFF
    pub can_track_time: bool,      // 최종 권한 (회사 + 개인)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplacedDevice {
    pub device_id: String,
    pub device_name: String,
    pub displaced_at: DateTime<Utc>,
}

// ───────────────────────── 2. Policy ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySnapshot {
    pub policy_version: i64,

    pub company_idle_threshold_seconds: Option<u64>,
    pub team_idle_threshold_seconds: Option<u64>,
    pub employee_idle_threshold_seconds: Option<u64>,
    pub effective_idle_threshold_seconds: u64,
    pub policy_scope: String, // COMPANY / TEAM / EMPLOYEE / DEFAULT

    pub lunch_start_time: String, // "HH:MM"
    pub lunch_end_time: String,
    pub lunch_allowed_minutes: u32,

    pub explanation_deadline_hours: u32,

    pub heartbeat_interval_seconds: u64,
    pub event_batch_interval_seconds: u64,

    pub can_track_time: bool,
}

impl PolicySnapshot {
    /// 서버 응답이 없을 때의 안전한 기본값.
    pub fn fallback(default_idle: u64, defaults: &crate::config::PolicyDefaults) -> Self {
        Self {
            policy_version: 0,
            company_idle_threshold_seconds: Some(default_idle),
            team_idle_threshold_seconds: None,
            employee_idle_threshold_seconds: None,
            effective_idle_threshold_seconds: default_idle,
            policy_scope: "DEFAULT".to_string(),
            lunch_start_time: defaults.default_lunch_start_time.clone(),
            lunch_end_time: defaults.default_lunch_end_time.clone(),
            lunch_allowed_minutes: defaults.default_lunch_allowed_minutes,
            explanation_deadline_hours: 48,
            heartbeat_interval_seconds: 180,
            event_batch_interval_seconds: 60,
            can_track_time: false,
        }
    }
}

// ───────────────────────── 3. Update check ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckRequest {
    pub current_version: String,
    pub os: String, // "windows" / "macos"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub minimum_required_version: String,
    pub update_required: bool,
    pub force_update: bool,
    pub download_url: String,
    pub release_note: String,
}

// ───────────────────────── 4. Heartbeat ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub company_id: String,
    pub employee_id: String,
    pub device_id: String,
    pub device_name: String,
    pub app_version: String,

    pub pc_status: String,
    pub last_activity_at: DateTime<Utc>,
    pub idle_seconds: u64,
    pub is_locked: bool,

    pub attendance_status: AttendanceStatus,
    pub can_track_time: bool,
    pub effective_idle_threshold_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub next_heartbeat_seconds: u64,
    pub policy_version: i64,
    pub can_track_time: bool,
    /// 서버에서 강제 로그아웃을 알리고 싶을 때 사용.
    #[serde(default)]
    pub force_logout: bool,
}

// ───────────────────────── 5. Events batch ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsBatch {
    pub company_id: String,
    pub employee_id: String,
    pub device_id: String,
    pub events: Vec<EventEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub event_id: String,
    pub event_type: String,
    pub event_time: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsBatchResponse {
    /// 서버가 정상적으로 받아들인 event_id 목록 (멱등성 확인용).
    pub accepted_event_ids: Vec<String>,
}

// ───────────────────────── 6. Explanations ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExplanation {
    pub segment_id: String,
    pub work_date: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: i64,
    pub segment_type: String,
    pub applied_idle_threshold_seconds: i64,
    pub explanation_deadline: Option<DateTime<Utc>>,
    pub explanation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationSubmit {
    pub segment_id: String,
    pub explanation_type: String,
    pub explanation_text: Option<String>,
    pub submitted_from: String, // "PC_APP"
}

// ───────────────────────── 7. Attendance ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSnapshot {
    pub attendance_status: AttendanceStatus,
    pub work_start_at: Option<DateTime<Utc>>,
    pub work_end_at: Option<DateTime<Utc>>,
}

// ───────────────────────── 8. 소명 사유 코드 ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplanationType {
    Meeting,
    PhoneCall,
    CustomerResponse,
    BusinessTrip,
    OutsideWork,
    Education,
    WorkWaiting,
    PcError,
    AppError,
    OtherWork,
    LunchBreak,
    Personal,
}

impl ExplanationType {
    pub const ALL: &'static [Self] = &[
        Self::Meeting,
        Self::PhoneCall,
        Self::CustomerResponse,
        Self::BusinessTrip,
        Self::OutsideWork,
        Self::Education,
        Self::WorkWaiting,
        Self::PcError,
        Self::AppError,
        Self::OtherWork,
        Self::LunchBreak,
        Self::Personal,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            Self::Meeting => "MEETING",
            Self::PhoneCall => "PHONE_CALL",
            Self::CustomerResponse => "CUSTOMER_RESPONSE",
            Self::BusinessTrip => "BUSINESS_TRIP",
            Self::OutsideWork => "OUTSIDE_WORK",
            Self::Education => "EDUCATION",
            Self::WorkWaiting => "WORK_WAITING",
            Self::PcError => "PC_ERROR",
            Self::AppError => "APP_ERROR",
            Self::OtherWork => "OTHER_WORK",
            Self::LunchBreak => "LUNCH_BREAK",
            Self::Personal => "PERSONAL",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Meeting => "회의",
            Self::PhoneCall => "전화 상담",
            Self::CustomerResponse => "고객 응대",
            Self::BusinessTrip => "출장",
            Self::OutsideWork => "외근",
            Self::Education => "교육",
            Self::WorkWaiting => "업무 지시 대기",
            Self::PcError => "PC 오류",
            Self::AppError => "앱 오류",
            Self::OtherWork => "기타 업무",
            Self::LunchBreak => "점심/휴게",
            Self::Personal => "개인 용무",
        }
    }
}
