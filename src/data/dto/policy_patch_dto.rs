//! ============================================================================
//! data::dto::policy_patch_dto — `PATCH /api/pc-agent/policy` 요청 DTO.
//! ============================================================================
//!
//! 2026-05-12 신규. 회사 관리자가 정책 부분 업데이트. 응답은 `PolicySnapshot`
//! (GET /policy 와 동일 스키마) 그대로 받음.
//!
//! ── 입력 범위 (기획자 결정 2026-05-12, 서버가 강제) ──
//!   - `idle_threshold_seconds`      : 180 ~ 3600
//!   - `lunch_start_time`            : "09:00" ~ "16:00"
//!   - `lunch_end_time`              : "09:00" ~ "16:00", 시작 < 끝, 길이 ≤ 3h
//!   - `lunch_allowed_minutes`       : 30 ~ 180
//!   - `explanation_deadline_hours`  : 24 ~ 168
//!   - `can_track_time`              : 운영자 수동 차단 오버라이드
//!
//! 클라가 1차 검증해서 서버 거부(`400 INVALID_FIELD`)를 사전 차단하고,
//! 서버는 동일 범위로 2차 검증 (단일 진실은 서버 측).
//!
//! ── 거부 필드 ──
//! `policy_version` / `policy_scope` / `cmpsid` / `ttmsid` / `temsid` / `empsid`
//! 는 서버가 자동 관리하므로 본 DTO 에 포함하지 않는다.

use serde::Serialize;

/// `PATCH /api/pc-agent/policy` 요청 본문.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyPatchRequest {
    pub requester_emp_sid: i64,
    /// 변경할 필드만 채워서 전송. `None` 필드는 직렬화 시 생략 (`skip_serializing_if`).
    pub patch: PolicyPatchFields,
    /// 운영자 메모. `PCAGT_POLICY_AUDIT.REASON` 에 저장.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 부분 업데이트 필드 모음 — 화이트리스트.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PolicyPatchFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_threshold_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lunch_start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lunch_end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lunch_allowed_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation_deadline_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_track_time: Option<bool>,
}

impl PolicyPatchFields {
    /// 클라 1차 유효성 검증.
    ///
    /// **2026-05-12 사용자 결정**: 범위/형식 검증 폐기. 사용자가 입력한 값 그대로
    /// 서버로 전송. 서버가 거부(`400 INVALID_FIELD` 등)하면 응답 메시지로 안내.
    /// 클라가 사전 차단 안 함 — 운영자가 자유롭게 값 시도 가능.
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    /// patch 가 비어있으면 송신 의미 없음 — UI 가드용.
    pub fn is_empty(&self) -> bool {
        self.idle_threshold_seconds.is_none()
            && self.lunch_start_time.is_none()
            && self.lunch_end_time.is_none()
            && self.lunch_allowed_minutes.is_none()
            && self.explanation_deadline_hours.is_none()
            && self.can_track_time.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patch_validates() {
        assert!(PolicyPatchFields::default().validate().is_ok());
    }

    #[test]
    fn any_value_passes_validate() {
        // 2026-05-12 결정: 클라 범위 검증 폐기, 서버가 단독 검증.
        let mut p = PolicyPatchFields::default();
        p.idle_threshold_seconds = Some(100);
        assert!(p.validate().is_ok());
        p.idle_threshold_seconds = Some(99999);
        assert!(p.validate().is_ok());
        p.lunch_start_time = Some("anything".into());
        p.lunch_end_time = Some("XX:YY".into());
        assert!(p.validate().is_ok());
    }
}
