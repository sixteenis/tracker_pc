//! ============================================================================
//! domain::service::session_caches — 세션 의존 도메인 캐시 일괄 clear 헬퍼.
//! ============================================================================
//!
//! 로그인 시작 직전과 로그아웃 시 호출. 같은 캐시 그룹을 한 곳에서 관리해서
//! 추후 도메인 서비스가 추가될 때 호출자(`user_usecase::finalize_session`,
//! `user_service::logout`) 두 곳을 매번 갱신할 필요 없게 한다.
//!
//! ── 처리 대상 (세션 의존) ─────────────────────────────────────────
//! - main_info_service     : V1 get_main2.jsp 응답 캐시
//! - subscription_service  : check_pay_use 응답 (pinpluse)
//! - explanation_type_service : 회사 사유 룩업 메모리 캐시
//! - work_status_service   : V1 get_workstatus.jsp result 캐시
//! - company_service       : 회사 식별/이름 캐시
//! - team_service          : 팀 식별/이름 캐시
//! - attendance_service    : V2 attendance 캐시
//! - policy_service        : 회사 정책 캐시
//!
//! UI 메모리 캐시(예: `ui::explanation_list_view::CACHE`) 는 layer 분리 위해
//! 호출자가 별도로 처리한다.

use super::{
    attendance_service, company_service, explanation_type_service, main_info_service,
    policy_service, subscription_service, team_service, work_status_service,
};

/// 세션 의존 도메인 캐시 일괄 clear.
pub fn clear_all() {
    main_info_service::clear();
    subscription_service::clear();
    explanation_type_service::clear();
    work_status_service::clear();
    company_service::clear();
    team_service::clear();
    attendance_service::clear();
    policy_service::clear();
}
