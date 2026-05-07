//! 인증 — 로그인 / 자동로그인 / 로그아웃.
//!
//! 토큰은 OS Credential Store (Windows: DPAPI 기반 Credential Manager,
//! macOS: Keychain) 에 저장한다. 비밀번호는 절대 보관하지 않는다.

pub mod token_store;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::{info, warn};

use crate::api::types::{
    AttendanceStatus, LoginRequest, LoginResponse, RefreshRequest, SubscriptionInfo,
};
use crate::api::ApiClient;
use crate::app::AppState;
use crate::db::auth_repo::{self, AuthRow};

/// 메모리에 들고 있는 활성 세션.
#[derive(Debug, Clone)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: DateTime<Utc>,

    pub company_id: String,
    pub employee_id: String,
    pub employee_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,

    pub subscription: SubscriptionInfo,
}

impl Session {
    fn from_response(resp: LoginResponse) -> Self {
        let expires_at =
            Utc::now() + chrono::Duration::seconds(resp.access_token_expires_in.max(60));
        Self {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            access_token_expires_at: expires_at,
            company_id: resp.company_id,
            employee_id: resp.employee_id,
            employee_name: resp.employee_name,
            team_id: resp.team_id,
            team_name: resp.team_name,
            subscription: resp.subscription,
        }
    }
}

/// 로그인 + 토큰/세션 영속화.
pub async fn login(state: &AppState, login_id: &str, password: &str, auto_login: bool) -> Result<()> {
    let req = LoginRequest {
        login_id: login_id.to_string(),
        password: password.to_string(),
        device_id: state.device.device_id.clone(),
        device_name: state.device.device_name.clone(),
        app_version: state.config.app.app_version.clone(),
    };
    // 비밀번호는 메모리에서 사용 후 즉시 drop. 로그에 절대 출력하지 않음.
    let resp = state.api.login(req).await.context("로그인 실패")?;
    apply_login_response(state, resp, auto_login).await
}

/// 시작 시 저장된 refresh_token 으로 자동로그인 시도.
/// 성공 시 `Some(())`, refresh 만료/저장된 토큰 없음 시 `None`, 그 외 에러는 `Err`.
pub async fn try_auto_login(state: &AppState) -> Result<Option<()>> {
    let row = match auth_repo::get(&state.db)? {
        Some(r) if r.auto_login => r,
        _ => return Ok(None),
    };
    let refresh = match token_store::load_refresh_token(&row.employee_id) {
        Ok(Some(t)) => t,
        Ok(None) => return Ok(None),
        Err(e) => {
            warn!(error = %e, "refresh token 로드 실패 — 수동 로그인 필요");
            return Ok(None);
        }
    };
    let req = RefreshRequest {
        refresh_token: refresh,
        device_id: state.device.device_id.clone(),
        device_name: state.device.device_name.clone(),
        app_version: state.config.app.app_version.clone(),
    };
    match state.api.refresh(req).await {
        Ok(resp) => {
            apply_login_response(state, resp, true).await?;
            Ok(Some(()))
        }
        Err(e) => {
            warn!(error = %e, "자동로그인 실패");
            Ok(None)
        }
    }
}

async fn apply_login_response(state: &AppState, resp: LoginResponse, auto_login: bool) -> Result<()> {
    // DB 식별 정보
    auth_repo::upsert(
        &state.db,
        &AuthRow {
            company_id: resp.company_id.clone(),
            employee_id: resp.employee_id.clone(),
            employee_name: resp.employee_name.clone(),
            team_id: resp.team_id.clone(),
            team_name: resp.team_name.clone(),
            device_id: state.device.device_id.clone(),
            device_name: state.device.device_name.clone(),
            auto_login,
        },
    )?;

    // 토큰 저장 (자동로그인 체크 시에만)
    if auto_login {
        if let Err(e) = token_store::save_refresh_token(&resp.employee_id, &resp.refresh_token) {
            warn!(error = %e, "refresh token 저장 실패 — 자동로그인이 다음 실행 시 동작하지 않을 수 있음");
        }
    } else {
        // 자동로그인 미선택 시 기존 토큰 제거.
        let _ = token_store::clear_refresh_token(&resp.employee_id);
    }

    // 정책/구독 → AppState
    let policy = resp.policy.clone();
    {
        let mut p = state.policy.write().unwrap();
        *p = policy.clone();
    }
    {
        let mut s = state.status.write().unwrap();
        s.can_track_time = resp.subscription.can_track_time && policy.can_track_time;
        s.effective_idle_threshold_seconds = policy.effective_idle_threshold_seconds;
        s.policy_scope = policy.policy_scope.clone();
        s.policy_version = policy.policy_version;
        s.attendance = AttendanceStatus::Unknown;
    }

    state.set_session(Some(Session::from_response(resp)));
    info!("로그인 완료 — 정책 적용");
    Ok(())
}

pub fn logout(state: &AppState) -> Result<()> {
    if let Some(sess) = state.session.read().unwrap().clone() {
        let _ = token_store::clear_refresh_token(&sess.employee_id);
    }
    auth_repo::clear(&state.db)?;
    state.set_session(None);
    if let Ok(mut s) = state.status.write() {
        s.can_track_time = false;
    }
    Ok(())
}
