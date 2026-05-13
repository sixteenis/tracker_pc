//! ============================================================================
//! data::dto::explanation_type_dto — `GET /api/pc-agent/explanation-types` 응답 DTO.
//! ============================================================================
//!
//! 회사 커스텀 소명사유 동적 목록 (Phase 1.b, 2026-05-12).
//! 기존 `ExplanationType` enum 하드코딩을 폐기하고 본 구조체로 전환.
//! 응답 캐시는 `settings` KV (`explanation_types_<cmpsid>` + `*_version_<cmpsid>`)
//! 에 보관, 오프라인 fallback 은 `domain::service::explanation_type_service` 의
//! 시스템 기본 12개.

use serde::{Deserialize, Serialize};

/// 회사별 활성 사유 한 건.
///
/// worker `GET /api/pc-agent/explanation-types` 응답에는 `exptype_sid`/`is_system`/
/// `is_active` 가 포함되지 않음 (서버 명세). CMS POST/PATCH 응답에는 포함되며
/// 클라가 무시하지 않고 받을 수 있도록 Optional 필드로 둠 (2026-05-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationType {
    /// `PCAGT_EXPLANATION_TYPE.EXPTYPE_SID` — PATCH/deactivate 의 path param.
    /// worker GET 응답에는 없음(None), CMS 응답에는 있음.
    #[serde(default)]
    pub exptype_sid: Option<i64>,
    /// 영문 코드. `PCAGT_EXPLANATION.EXPLANATION_TYPE` 에 저장되는 값.
    pub code: String,
    /// UI 표시명.
    pub label: String,
    /// 정렬 순서 (오름차순).
    pub sort_order: i32,
    /// 아이콘 식별자 (선택). 디자인 가이드 확정 전엔 None.
    #[serde(default)]
    pub icon: Option<String>,
    /// true 면 자유 텍스트 입력 강제.
    #[serde(default)]
    pub requires_text: bool,
    /// 시스템 시드 여부. CMS 응답에서 받음. 비활성화는 가능하지만 일부 필드 잠금.
    #[serde(default)]
    pub is_system: bool,
    /// 보호 row 여부 (예: 'OTHER' 기타). true 면 CMS 비활성화 거부 (409 PROTECTED_TYPE).
    /// 클라는 회사 사유 목록 UI 에서 비활성화 버튼 자체를 가린다.
    #[serde(default)]
    pub is_protected: bool,
}

/// 매칭된 스코프 키 (디버그/관제용).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationScopeKeys {
    pub cmpsid: i64,
    #[serde(default)]
    pub ttmsid: Option<i64>,
    #[serde(default)]
    pub temsid: Option<i64>,
}

/// `/explanation-types` 응답 전체.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationTypesResponse {
    /// 매칭된 스코프. MVP 는 항상 `"COMPANY"`. Phase 3 에서 `"TEAM"` / `"TOPTEAM"`.
    pub scope: String,
    pub scope_keys: ExplanationScopeKeys,
    /// 활성 사유 배열. 서버가 `sort_order` 오름차순으로 정렬해서 응답.
    pub types: Vec<ExplanationType>,
    /// `MAX(UPD_DT)` epoch 초. user-info 응답의 `explanation_types_version` 과 비교.
    pub version: i64,
    /// 이번 호출에서 자동 시드(회사 첫 호출) 발생 여부.
    #[serde(default)]
    pub seeded: bool,
}
