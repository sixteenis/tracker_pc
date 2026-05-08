//! ============================================================================
//! domain::service::company_service — 회사 정보 (현재 mock).
//! ============================================================================
//!
//! 로그인 응답에 `cmpsid` / `cmpname` 이 들어 있어 `User` 와 함께 즉시 채울 수
//! 있다. 별도 회사 상세 조회 API 가 합의되면 `data::repository::company_repository`
//! 에 fetch 함수를 추가하고 본 service 가 그걸 호출하도록 변경.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::data::repository::company_repository;
use crate::domain::model::company::Company;

static CURRENT: Lazy<RwLock<Option<Company>>> = Lazy::new(|| RwLock::new(None));

/// 현재 사용자의 회사 정보.
pub fn current() -> Option<Company> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

/// 로그인 성공 시 user_service 가 호출 — 회사 정보 캐싱.
pub fn set(company: Company) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(company);
    }
}

/// 로그아웃 시 호출.
pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

/// 회사 상세 정보 조회 (현재 mock 반환).
pub fn fetch_mock() -> Company {
    company_repository::mock_company()
}
