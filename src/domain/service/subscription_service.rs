//! 회사 요금제 권한 도메인 서비스 — `pinpluse` 게이트 결과 캐싱.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::domain::model::subscription::Subscription;

static CURRENT: Lazy<RwLock<Option<Subscription>>> = Lazy::new(|| RwLock::new(None));

/// 현재 회사 요금제 권한.
pub fn current() -> Option<Subscription> {
    CURRENT.read().ok().and_then(|g| *g)
}

/// usecase 가 `check_pay_use` 호출 후 갱신.
pub fn set(sub: Subscription) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(sub);
    }
}

/// 로그아웃 / 진입 차단 시 호출.
pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

/// PIN+ 권한 보유 여부 — UI 라우팅에서 빠르게 확인.
pub fn pin_plus_active() -> bool {
    current().map(|s| s.pin_plus_active).unwrap_or(false)
}
