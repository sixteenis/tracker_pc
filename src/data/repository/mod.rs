//! ============================================================================
//! data::repository — DTO ↔ 도메인 모델 변환 + 데이터 진입점.
//! ============================================================================
//!
//! 외부(`domain::service`) 에 노출하는 함수들은 항상 도메인 모델만 반환한다.
//! DTO 는 본 모듈 안에서만 다뤄진다.
//!
//! - `auth_repository`       : 실제 서버 호출 (login / try_auto_login / logout local)
//! - `policy_repository`     : 현재 mock — 신규 서버 정책 명세 합의 후 실제 호출로 교체
//! - `company_repository`    : 현재 mock
//! - `team_repository`       : 현재 mock
//! - `attendance_repository` : 현재 mock

pub mod attendance_repository;
pub mod auth_repository;
pub mod company_repository;
pub mod main_info_repository;
pub mod policy_repository;
pub mod subscription_repository;
pub mod team_repository;
