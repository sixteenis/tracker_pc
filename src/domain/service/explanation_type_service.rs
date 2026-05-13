//! ============================================================================
//! domain::service::explanation_type_service — 회사 커스텀 소명사유 관리.
//! ============================================================================
//!
//! 진실 소스는 서버 `/api/pc-agent/explanation-types` 응답. **디스크 캐시는 두지 않는다**
//! (사용자 결정 2026-05-12 — 다른 PC 에서 CMS 수정한 결과가 본 PC 에 늦게 반영되거나
//! 재시작 후 stale 캐시가 보이는 문제 회피).
//!
//! 클라는 다음 2단계 fast path 를 사용한다:
//!   1) **메모리 캐시** (`CURRENT` RwLock) — UI 가 매 프레임 접근.
//!   2) **시스템 기본 12개 fallback** (`system_default_types`) — 메모리 캐시 미설정 시.
//!
//! 호출 흐름:
//!   - 로그인 직후 `user_usecase::finalize_session` 가 `list_explanation_types` 1회 호출 →
//!     `store_response` 로 메모리 캐시 설정.
//!   - `sync::user_info_sync` 가 매 폴링 사이클(적응형 5분~1시간)에 무조건 재호출 →
//!     `store_response` 로 메모리 캐시 갱신.
//!   - CMS CRUD 직후 `settings_view::refresh_explanation_cache` 가 즉시 호출.
//!   - UI 가 `current_types()` 로 동기 조회.
//!
//! ── fallback 동작 (offline) ───────────────────────────────────────────
//! 첫 로그인 직후 네트워크 실패 또는 응답 미수신 시 `current_types()` 가
//! `system_default_types()` 반환. 회사가 사유를 커스텀한 뒤 fallback 사유로
//! 제출하면 서버가 `400 INVALID_EXPLANATION_TYPE` 반환 — UI 가 안내 모달.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::data::dto::{ExplanationType, ExplanationTypesResponse};

/// 메모리 캐시 — UI fast path.
static CURRENT: Lazy<RwLock<Option<Vec<ExplanationType>>>> = Lazy::new(|| RwLock::new(None));

/// 메모리 캐시의 현재 사유 목록. 비어있으면 시스템 기본 12개 fallback.
/// UI 가 매 프레임 호출.
pub fn current_types() -> Vec<ExplanationType> {
    CURRENT
        .read()
        .ok()
        .and_then(|g| g.as_ref().cloned())
        .unwrap_or_else(system_default_types)
}

/// 로그아웃 시 호출 — 메모리 캐시 비움.
pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

/// 서버 응답 수신 시 호출 — 메모리 캐시 갱신.
pub fn store_response(resp: &ExplanationTypesResponse) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(resp.types.clone());
    }
}

/// 시스템 기본 13개 — 서버 자동 시드와 동일. 메모리 캐시 미설정 시 fallback.
/// 'OTHER' (기타) 는 is_protected=true — 회사 관리자가 비활성화 불가, 사용자가 콤보에서
/// 선택하면 자유 라벨(1~50자) 을 직접 입력하는 통로. 그 외 12개는 일반 시스템 시드.
pub fn system_default_types() -> Vec<ExplanationType> {
    vec![
        et("MEETING", "회의", 10, false, false),
        et("PHONE_CALL", "전화상담", 20, false, false),
        et("CUSTOMER_RESPONSE", "고객대응", 30, false, false),
        et("BUSINESS_TRIP", "출장", 40, false, false),
        et("OUTSIDE_WORK", "외근", 50, true, false),
        et("EDUCATION", "교육", 60, false, false),
        et("WORK_WAITING", "업무 대기", 70, false, false),
        et("PC_ERROR", "PC 오류", 80, false, false),
        et("APP_ERROR", "앱 오류", 90, false, false),
        et("OTHER_WORK", "기타 업무", 100, true, false),
        et("LUNCH_BREAK", "점심시간", 110, false, false),
        et("PERSONAL", "개인 사유", 120, true, false),
        et("OTHER", "기타", 999, false, true),
    ]
}

fn et(code: &str, label: &str, sort_order: i32, requires_text: bool, is_protected: bool) -> ExplanationType {
    ExplanationType {
        exptype_sid: None,
        code: code.to_string(),
        label: label.to_string(),
        sort_order,
        icon: None,
        requires_text,
        is_system: true,
        is_protected,
    }
}
