//! ============================================================================
//! domain::usecase::user_usecase — 사용자 진입(로그인 / 자동로그인) 흐름.
//! ============================================================================
//!
//! 두 진입 동작(`login`, `auto_login`) 의 차이는 자격증명을 어디에서 가져오느냐
//! 한 가지뿐. 인증 이후의 5단계는 양쪽이 동일하므로 `finalize_session` 헬퍼로
//! 추출했다.
//!
//! 진입점:
//!   - `login(state, email, password, auto_login)` — 사용자 입력 평문 자격증명
//!   - `auto_login(state)` — keyring + DB `auth_repo` 의 자동로그인 행 사용
//!
//! 공통 파이프라인 (인증 성공 후):
//!   1. `subscription_repository::fetch`  ← `check_pay_use.jsp`
//!   2. `main_info_repository::fetch`     ← `get_main2.jsp`
//!   3. `auth_repository::persist_session` (DB / keyring / 회사·팀 캐시 / status)
//!   4. 도메인 서비스 갱신 (`subscription_service`, `main_info_service`, `user_service`)
//!   5. PIN+ 미사용이면 `state.status.can_track_time` 을 false 로 강제 (AND 합성)
//!
//! PIN+ 미사용도 로그인 자체는 통과시키며, 메인 화면에서 헤더 배지 + 입력 폴링
//! 차단으로 처리됨 (UI 가 `subscription_service::pin_plus_active()` 분기).

use anyhow::Result;
use chrono::Utc;
use tracing::info;

use crate::app::AppState;
use crate::data::local::events_repo;
use crate::data::repository::auth_repository::{self, Authenticated};
use crate::data::repository::{main_info_repository, subscription_repository};
use crate::domain::model::user::User;
use crate::domain::service::{
    explanation_type_service, main_info_service, session_caches, subscription_service,
    user_service, work_status_service,
};

/// 사용자 입력 평문 자격증명으로 로그인. UI 로그인 버튼이 호출자.
pub async fn login(
    state: &AppState,
    email: &str,
    password: &str,
    auto_login: bool,
) -> Result<User> {
    let authed = auth_repository::authenticate(state, email, password).await?;
    finalize_session(state, authed, auto_login).await
}

/// keyring 자격증명으로 자동로그인. 앱 시작 시 한 번 호출.
/// `Ok(None)` — 자동로그인 대상 없음(자격증명 없거나 인증 실패 후 폐기됨).
pub async fn auto_login(state: &AppState) -> Result<Option<User>> {
    let authed = match auth_repository::try_auto_authenticate(state).await? {
        Some(a) => a,
        None => return Ok(None),
    };
    let user = finalize_session(state, authed, true).await?;
    Ok(Some(user))
}

// ─────────────────────────── 내부 ───────────────────────────

/// 인증 성공 직후의 공통 5단계.
async fn finalize_session(
    state: &AppState,
    authed: Authenticated,
    auto_login: bool,
) -> Result<User> {
    // 다른 사용자 / 이전 세션 잔재로부터의 데이터 노출 방지.
    // 도메인 캐시(직원·회사·팀·출근·정책·요금제·소명사유·workstatus·main_info) 전부 비우고
    // 아래 fetch / 폴링이 서버 응답으로 새로 채운다. UI 캐시는 호출자가 처리
    // (logout 패턴과 일관 — `user_service::logout` 도 UI 캐시는 호출자 책임).
    session_caches::clear_all();

    let subscription =
        subscription_repository::fetch(state, authed.user.company_id, authed.user.member_id)
            .await?;

    let team_id = authed.user.team_id.unwrap_or(0);
    let main_info = main_info_repository::fetch(
        state,
        authed.user.employee_id,
        authed.user.company_id,
        authed.user.team_template_id,
        team_id,
    )
    .await?;

    auth_repository::persist_session(state, &authed, auto_login)?;

    subscription_service::set(subscription);
    main_info_service::set(main_info);
    let user = authed.user;
    user_service::set_current(state, user.clone());

    apply_subscription_gate(state, subscription.pin_plus_active);

    // explanation_types 로그인 직후 1회 호출 — 디스크 캐시 없이 매번 서버 진실 소스.
    // 실패해도 로그인 자체는 통과시키고 fallback 12개 사용 → user_info_sync 가 다음
    // 사이클에 재시도. (2026-05-12 사용자 결정: 디스크 캐시 폐기)
    match state.api.list_explanation_types(user.employee_id).await {
        Ok(resp) => {
            explanation_type_service::store_response(&resp);
            info!(version = resp.version, count = resp.types.len(), "explanation_types 로그인 직후 로드");
        }
        Err(e) => tracing::warn!(error = %e, "explanation_types 로그인 직후 로드 실패 — fallback 사용"),
    }

    // user-info 로그인 직후 1회 호출 — UI 가 첫 진입에 attendance/팀 정보를 곧바로 표시.
    // user_info_sync 가 같은 사이클을 곧 다시 돌지만, 초기 sleep + 폴링 간격으로 인한 지연을
    // 없애려는 목적. 실패해도 user_info_sync 가 재시도하므로 fatal X.
    match state.api.get_user_info(user.employee_id).await {
        Ok(snap) => {
            if let Ok(mut s) = state.status.write() {
                s.attendance = snap.attendance.attendance_status;
                s.last_user_info_sync_at = Some(Utc::now());
            }
            info!(attendance = ?snap.attendance.attendance_status, "user-info 로그인 직후 로드");
        }
        Err(e) => tracing::warn!(error = %e, "user-info 로그인 직후 로드 실패 — user_info_sync 가 재시도"),
    }

    // get_workstatus.jsp 로그인 직후 1회 호출 — UI 출근 카드 라벨 + idle_detector 게이트
    // 진실 소스. 자동로그인 직후 5초 이내 우선 호출 권고 (기획 결정 2026-05-12).
    match state.api.get_work_status(user.employee_id).await {
        Ok(resp) => {
            work_status_service::set_result(resp.result);
            info!(result = resp.result, "get_workstatus 로그인 직후 로드");
        }
        Err(e) => tracing::warn!(error = %e, "get_workstatus 로그인 직후 로드 실패 — user_info_sync 가 재시도"),
    }

    // 소명 리스트 로그인 직후 1회 호출 — 메인 화면 진입 첫 프레임부터 정확한 카운트/통계.
    // 캐시 비어있으면 status_view 가 ensure_cache_loaded 로 trigger 하지만, 비동기 spawn
    // 이라 첫 프레임은 비어있게 됨 → 깜박임 회피 위해 finalize 안에서 동기 fetch + store.
    // 실패해도 fatal X — fallback 으로 로컬 카운트 사용. (2026-05-13)
    match state.api.list_explanations(user.employee_id).await {
        Ok(items) => {
            let count = items.len();
            crate::ui::explanation_list_view::store_response_for(user.employee_id, items);
            info!(count, "worktime-explanations 로그인 직후 로드");
        }
        Err(e) => tracing::warn!(error = %e, "worktime-explanations 로그인 직후 로드 실패 — 메인 화면 진입 시 재시도"),
    }

    // policy 로그인 직후 1회 호출 — idle_detector 가 fallback 임계치(테스트값)로 동작하는
    // 문제 회피. policy_sync 가 30분 주기라 첫 호출까지 그 시간 동안 잘못된 임계치
    // 사용. 실패해도 policy_sync 가 재시도 (이전 값 또는 default 유지).
    match state.api.get_policy(user.employee_id).await {
        Ok(p) => {
            if let Ok(mut s) = state.status.write() {
                s.effective_idle_threshold_seconds = p.effective_idle_threshold_seconds;
                s.policy_scope = p.policy_scope.clone();
                s.policy_version = p.policy_version;
                s.can_track_time = s.can_track_time && p.can_track_time;
                s.last_policy_sync_at = Some(Utc::now());
            }
            if let Ok(mut policy) = state.policy.write() {
                *policy = p.clone();
            }
            info!(
                effective_idle_threshold_seconds = p.effective_idle_threshold_seconds,
                policy_scope = %p.policy_scope,
                policy_version = p.policy_version,
                "policy 로그인 직후 로드"
            );
        }
        Err(e) => tracing::warn!(error = %e, "policy 로그인 직후 로드 실패 — policy_sync 가 재시도"),
    }

    // PRESENCE — LOGIN_SUCCESS (수동) / AUTO_LOGIN_SUCCESS (자동). 서버가 PCAGT_PRESENCE_LOG 매핑.
    let event_type = if auto_login { "AUTO_LOGIN_SUCCESS" } else { "LOGIN_SUCCESS" };
    let _ = events_repo::enqueue(
        &state.db,
        event_type,
        Utc::now(),
        &serde_json::json!({
            "device_id": state.device.device_id,
            "app_version": state.config.app.app_version,
        }),
    );

    info!(pin_plus = subscription.pin_plus_active, "사용자 진입 완료");
    Ok(user)
}

/// PIN+ 미사용이면 추적/이벤트 차단 게이트.
/// 다른 게이트(정책 응답) 결과를 보존하기 위해 AND 로 합성.
fn apply_subscription_gate(state: &AppState, pin_plus_active: bool) {
    if let Ok(mut s) = state.status.write() {
        s.can_track_time = s.can_track_time && pin_plus_active;
    }
}
