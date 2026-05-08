//! ============================================================================
//! domain::service::user_service — 로그인 사용자 도메인 서비스.
//! ============================================================================
//!
//! 앱 어디에서든:
//!   - `current()` — 로그인된 `User` 사본 (`Option`)
//!   - `is_logged_in()` — 빠른 boolean
//!   - `login(state, email, password, auto)` — 평문 자격증명으로 로그인 시도
//!   - `try_auto_login(state)` — keyring 자격증명으로 자동 로그인
//!   - `logout(state)` — 모든 사용자 상태 제거
//!
//! 로그인 성공 시 `User` 가 본 모듈 내부 `RwLock<Option<User>>` 에 보관되며,
//! 동시에 `AppState::session` 에도 동기화된다 (기존 sync/UI 호환).

use std::sync::RwLock;

use anyhow::Result;
use once_cell::sync::Lazy;
use tracing::info;

use crate::app::AppState;
use crate::data::repository::auth_repository;
use crate::domain::model::user::User;
use crate::platform::credential_store;

/// 프로세스 전역 회원 정보 슬롯.
static CURRENT: Lazy<RwLock<Option<User>>> = Lazy::new(|| RwLock::new(None));

/// 현재 로그인된 사용자 사본.
pub fn current() -> Option<User> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

/// 로그인 여부.
pub fn is_logged_in() -> bool {
    CURRENT.read().map(|g| g.is_some()).unwrap_or(false)
}

/// 사용자가 입력한 평문 자격증명으로 로그인. 성공 시 도메인 / AppState 모두 갱신.
pub async fn login(
    state: &AppState,
    email: &str,
    password: &str,
    auto_login: bool,
) -> Result<User> {
    let user = auth_repository::login(state, email, password, auto_login).await?;
    set_current(state, user.clone());
    info!(email = %mask_email(&user.email), "도메인: 사용자 로그인 완료");
    Ok(user)
}

/// 자동 로그인 — keyring 의 자격증명으로 같은 login 엔드포인트 재호출.
/// 결과:
///   - `Ok(true)`  : 성공 — UI 라우팅 분기
///   - `Ok(false)` : 자격증명 없음 / 실패 (자격증명은 폐기됨)
///   - `Err(_)`    : 네트워크 등 일시 장애
pub async fn try_auto_login(state: &AppState) -> Result<bool> {
    match auth_repository::try_auto_login(state).await? {
        Some(user) => {
            set_current(state, user);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 모든 사용자 상태 제거 (메모리 / DB / keyring / AppState).
pub fn logout(state: &AppState) -> Result<()> {
    let _ = credential_store::clear();
    auth_repository::clear_local(state)?;
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
    state.set_session(None);
    if let Ok(mut s) = state.status.write() {
        s.can_track_time = false;
    }
    Ok(())
}

// ─────────────────────────── 내부 ───────────────────────────

fn set_current(state: &AppState, user: User) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(user.clone());
    }
    state.set_session(Some(user));
}

fn mask_email(email: &str) -> String {
    if let Some((local, domain)) = email.split_once('@') {
        let head: String = local.chars().take(3).collect();
        format!("{head}***@{domain}")
    } else {
        "***".to_string()
    }
}
