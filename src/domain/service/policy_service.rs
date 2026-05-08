//! ============================================================================
//! domain::service::policy_service — 정책 도메인 서비스 (현재 mock).
//! ============================================================================
//!
//! 신규 서버의 정책 엔드포인트가 합의되면 `data::repository::policy_repository`
//! 가 실제 호출로 교체되고, 본 서비스 코드는 그대로 유지된다.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::data::repository::policy_repository;
use crate::domain::model::policy::Policy;

static CURRENT: Lazy<RwLock<Option<Policy>>> = Lazy::new(|| RwLock::new(None));

pub fn current() -> Option<Policy> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

pub fn set(policy: Policy) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(policy);
    }
}

pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

/// 로그인 직후 / 주기 동기화 시점에 호출. 현재는 mock 반환만.
pub fn fetch_mock() -> Policy {
    policy_repository::mock_policy()
}
