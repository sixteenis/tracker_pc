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

// 로그인 DTO 는 `data::dto::login_dto` 로 이동됨.

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
            explanation_deadline_hours: defaults.default_explanation_deadline_hours,
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
    #[serde(default)]
    pub work_date: String,
    /// 진행 중(open) segment 는 서버 응답에서 null 일 수 있음 → Option.
    /// 명세 [[T-20260512-04_소명내역_응답값_명세_전달]] §2.3: start_time NOT NULL 이지만
    /// fault-tolerant 위해 Option 유지.
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,
    /// 진행 중(open) segment 는 NULL. 명세 §2.3: NULL 허용.
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
    /// 진행 중(open) segment 는 NULL (명세 §2.3 — NULL 허용).
    /// 28KB 응답 안에 한 row 라도 `null` 이면 전체 파싱 실패하던 사고가
    /// 2026-05-12 발생 → Option 으로 fault-tolerant 처리.
    #[serde(default)]
    pub duration_seconds: Option<i64>,
    #[serde(default)]
    pub segment_type: String,
    #[serde(default)]
    pub applied_idle_threshold_seconds: i64,
    #[serde(default)]
    pub explanation_deadline: Option<DateTime<Utc>>,
    #[serde(default)]
    pub explanation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationSubmit {
    /// 근로자 EMPSID. 서버가 segment 소유자 검증에 사용.
    pub employee_id: String,
    /// 회사 CMPSID — 서버에 segment 가 없을 때 upsert 용.
    pub company_id: String,
    /// 디바이스 UUID — segment upsert 용.
    pub device_id: String,
    pub segment_id: String,
    pub explanation_type: String,
    pub explanation_text: Option<String>,
    /// `explanation_type == "OTHER"` (기타) 일 때 사용자가 직접 입력한 유형명 (1~50자).
    /// 다른 사유에서는 None — 서버가 무시(NULL 저장).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_type_label: Option<String>,
    pub submitted_from: String, // "PC_APP"

    // segment 메타 — 서버에 같은 segment_id 가 없으면 이 정보로 즉시 upsert.
    pub work_date: String, // "YYYY-MM-DD"
    pub segment_type: String, // PC_IDLE / PC_LOCKED / ...
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub applied_idle_threshold_seconds: i64,
    pub policy_scope: String, // COMPANY / TEAM / EMPLOYEE / DEFAULT
}

// ───────────────────────── 7. Attendance ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSnapshot {
    pub attendance_status: AttendanceStatus,
    pub work_start_at: Option<DateTime<Utc>>,
    pub work_end_at: Option<DateTime<Utc>>,
}

// 소명 사유 코드 — 2026-05-12 회사 커스텀(Phase 1.b) 전환으로 enum 폐기.
// 신규 동적 구조체는 `data::dto::explanation_type_dto::ExplanationType`,
// 오프라인 fallback / 시스템 기본 12개는
// `domain::service::explanation_type_service::system_default_types()`.
