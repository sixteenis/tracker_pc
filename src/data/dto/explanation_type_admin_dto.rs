//! ============================================================================
//! data::dto::explanation_type_admin_dto — 회사 커스텀 소명사유 CMS CRUD DTO.
//! ============================================================================
//!
//! 2026-05-12 도입. 회사 관리자(`Emply.Author>=5`) 가 사유를 추가/수정/비활성화
//! 할 수 있는 CMS endpoint 4종의 요청·응답 DTO.
//!
//! 서버 명세: [[API_명세_핀플_PC_Agent]] §12~§15.
//! 권한 검증·활성 셋 ≥ 1 가드·`DUPLICATE_CODE` 검증은 모두 서버 단독.
//!
//! ── dev 전용 사용처 ────────────────────────────────────────────
//! 현재 클라는 release 빌드의 운영 CMS 화면 미보유. dev 회사설정 옆 탭에서
//! 테스트 호출 용도로만 사용 (`#[cfg(debug_assertions)]`).

use serde::{Deserialize, Serialize};

/// CMS POST 추가 요청 — 2026-05-12 시그니처 변경:
///   - `cmpsid` 추가 (요청자 회사 ID)
///   - `code` 제거 (서버 자동 생성 — 응답 body 에서 받음)
#[derive(Debug, Clone, Serialize)]
pub struct CreateExplanationTypeRequest {
    pub cmpsid: i64,
    pub requester_emp_sid: i64,
    pub label: String,
    pub sort_order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub requires_text: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PatchExplanationTypeRequest {
    pub requester_emp_sid: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_text: Option<bool>,
}

/// CMS PATCH-deactivate 요청 — 2026-05-12 시그니처 변경: `cmpsid` 추가.
#[derive(Debug, Clone, Serialize)]
pub struct DeactivateExplanationTypeRequest {
    pub cmpsid: i64,
    pub requester_emp_sid: i64,
}

/// `GET /api/cms/pc-agent/explanation-types/usage?days=` 응답 한 항목.
#[derive(Debug, Clone, Deserialize)]
pub struct ExplanationUsageEntry {
    pub code: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub distinct_users: i64,
}
