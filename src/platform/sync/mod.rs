//! ============================================================================
//! sync — 백그라운드 동기화 작업 (서버 ↔ 로컬).
//! ============================================================================
//!
//! 4개의 독립 tokio task:
//!
//! | 모듈                | 주기            | 동작                                 |
//! |---------------------|----------------|--------------------------------------|
//! | event_sync          | 1분             | local_events 큐 → 서버 배치 전송. PRESENCE(LOGIN_SUCCESS/AUTO_LOGIN_SUCCESS/LOGOUT/APP_STOPPED/PC_SHUTDOWN_DETECTED) 도 같은 채널로 전송 — 서버가 PCAGT_PRESENCE_LOG 로 매핑 |
//! | policy_sync         | 30분            | 정책 재조회 (관리자가 변경했을 수도) |
//! | update_check        | 12시간          | 앱 업데이트 정보 폴링                |
//! | **user_info_sync**  | **1h / 5min**   | **check_pay_use(V1) + user-info(V2) + main_info(V1 get_main2.jsp) 통합 폴링 (적응형). `attendance == WORKING` ⇒ 1시간, 그 외 ⇒ 5분. force_logout 신호 처리. policy_version 비교 트리거(예정).** |
//!
//! 각 task 는 `state.session.read()` → `Option<Session>` 으로 인증 확인 후
//! 없으면 sleep 후 재시도. 로그인 → 로그아웃 → 재로그인 시에도 재시작 불필요.
//!
//! `heartbeat` / `attendance_sync` 는 폐기됨. heartbeat 의 책임은
//! events(PRESENCE_LOG) + user-info(force_logout/policy_version) 로 분산 흡수.
//! attendance_sync 는 user_info_sync 로 흡수.
//!
//! TODO(2차): "지금 동기화" 수동 트리거 채널 — 현재는 모든 task 가 자기 주기로만
//! 동작. UI 의 "↻ 지금 동기화" 버튼이 작동하려면 mpsc/broadcast 채널 필요.

pub mod attendance_sync; // deprecated: user_info_sync 로 대체. 호환을 위해 유지.
pub mod event_sync;
pub mod policy_sync;
pub mod update_check;
pub mod user_info_sync;

use std::sync::Arc;

use crate::app::AppState;

/// 앱 시작 시 한 번 호출 — 4개 동기화 task 를 모두 띄운다.
pub fn spawn_all(state: Arc<AppState>) {
    let s = state.clone();
    state.runtime.spawn(async move { event_sync::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { policy_sync::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { update_check::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { user_info_sync::run(s).await });
}
