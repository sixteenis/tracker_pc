//! ============================================================================
//! domain::service — 도메인 데이터 진입점 (싱글톤 + 메모리 캐시).
//! ============================================================================
//!
//! UI / 다른 도메인은 항상 이 service 모듈을 거쳐서 데이터를 얻는다.
//! 서비스가 내부적으로:
//!   1) `data::repository` 에 위임해서 데이터를 가져오고
//!   2) `RwLock<Option<Domain>>` 에 보관하고
//!   3) UI 의 동기 호출에 즉시 응답
//!
//! 일부 도메인(`policy`, `company`, `attendance`) 은 아직 실제 서버 연동이
//! 안 된 상태이므로 repository 가 mock 데이터를 반환한다 — 서비스 코드는
//! 실/mock 을 구분하지 않는다.

pub mod attendance_service;
pub mod company_service;
pub mod explanation_type_service;
pub mod main_info_service;
pub mod policy_service;
pub mod subscription_service;
pub mod team_service;
pub mod user_service;
pub mod work_status_service;
