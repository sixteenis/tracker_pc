//! ============================================================================
//! domain::service::user_service — 로그인 사용자 도메인 서비스.
//! ============================================================================
//!
//! 본 서비스는 "현재 로그인된 사용자" 슬롯의 보관자 역할만 한다.
//! 실제 로그인/자동로그인 흐름의 오케스트레이션(인증 → 요금제 → 메인정보)은
//! `domain::usecase::user_usecase::{login, auto_login}` 가 담당하며, 이들이
//! 본 서비스의 `set_current` / `logout` 을 호출한다.

use std::sync::RwLock;

use anyhow::Result;
use chrono::Utc;
use once_cell::sync::Lazy;
use tracing::info;

use crate::app::AppState;
use crate::data::local::events_repo;
use crate::data::repository::auth_repository;
use crate::domain::model::user::User;
use crate::domain::service::session_caches;
use crate::platform::credential_store;

/// 로그아웃 원인 — events 채널의 `LOGOUT` 이벤트 `reason` 필드 값.
/// 서버가 PCAGT_PRESENCE_LOG.REASON 으로 매핑.
#[derive(Debug, Clone, Copy)]
pub enum LogoutReason {
    /// 사용자가 UI 에서 명시적으로 로그아웃 (설정 화면, Disabled 화면).
    UserAction,
    /// `GET /user-info` 응답 `force_logout=true` 수신 (인증 무효화 — 이메일/비번 변경, 퇴사 등).
    ForceLogout,
}

impl LogoutReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserAction => "USER_ACTION",
            Self::ForceLogout => "FORCE_LOGOUT",
        }
    }
}

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

/// 로그인 usecase 성공 시 호출 — 메모리 / AppState 양쪽 갱신.
pub fn set_current(state: &AppState, user: User) {
    info!(email = %mask_email(&user.email), "도메인: 사용자 로그인 완료");
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(user.clone());
    }
    state.set_session(Some(user));
}

/// 모든 사용자 상태 제거 (메모리 / DB / keyring / AppState / 메인정보 / 요금제) + LOGOUT 이벤트 enqueue.
///
/// `reason` 은 PRESENCE_LOG 매핑용 — `UserAction` (UI 로그아웃) / `ForceLogout` (서버 force_logout).
/// 인증 실패(자동로그인 시도 실패) 처럼 **세션이 시작된 적 없는** 케이스는 본 함수 호출 X
/// (`auth_repository::try_auto_authenticate` 가 직접 자격증명/DB row 만 폐기).
pub fn logout(state: &AppState, reason: LogoutReason) -> Result<()> {
    // LOGOUT 이벤트 — credential / DB 정리 전에 enqueue (events 큐는 별도 테이블이라 정리에 영향 없음).
    let _ = events_repo::enqueue(
        &state.db,
        "LOGOUT",
        Utc::now(),
        &serde_json::json!({
            "reason": reason.as_str(),
            "device_id": state.device.device_id,
            "app_version": state.config.app.app_version,
        }),
    );

    let _ = credential_store::clear();
    auth_repository::clear_local(state)?;
    // 세션 의존 도메인 캐시 일괄 clear (직원·회사·팀·출근·정책·요금제·소명사유·workstatus·main_info).
    // UI 캐시는 호출자 (settings_view, disabled_view, user_info_sync) 책임.
    session_caches::clear_all();
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
    state.set_session(None);
    if let Ok(mut s) = state.status.write() {
        s.can_track_time = false;
    }
    Ok(())
}

fn mask_email(email: &str) -> String {
    if let Some((local, domain)) = email.split_once('@') {
        let head: String = local.chars().take(3).collect();
        format!("{head}***@{domain}")
    } else {
        "***".to_string()
    }
}
