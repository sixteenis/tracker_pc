//! ============================================================================
//! sync — 백그라운드 동기화 작업 (서버 ↔ 로컬).
//! ============================================================================
//!
//! 5개의 독립 tokio task:
//!
//! | 모듈                | 주기   | 동작                                 |
//! |---------------------|--------|--------------------------------------|
//! | heartbeat           | 3분    | PC 상태 보고, 정책 버전 동기화       |
//! | event_sync          | 1분    | local_events 큐 → 서버 배치 전송     |
//! | policy_sync         | 30분   | 정책 재조회 (관리자가 변경했을 수도) |
//! | update_check        | 12시간 | 앱 업데이트 정보 폴링                |
//! | attendance_sync     | 5분    | 출근 상태 폴링 (스마트폰 앱이 변경)  |
//!
//! 각 task 는 `state.session.read()` → `Option<Session>` 으로 인증 확인 후
//! 없으면 sleep 후 재시도. 로그인 → 로그아웃 → 재로그인 시에도 재시작 불필요.
//!
//! TODO(2차): "지금 동기화" 수동 트리거 채널 — 현재는 모든 task 가 자기 주기로만
//! 동작. UI 의 "↻ 지금 동기화" 버튼이 작동하려면 mpsc/broadcast 채널 필요.

pub mod attendance_sync;
pub mod event_sync;
pub mod heartbeat;
pub mod policy_sync;
pub mod update_check;

use std::sync::Arc;

use crate::app::AppState;

/// 앱 시작 시 한 번 호출 — 5개 동기화 task 를 모두 띄운다.
pub fn spawn_all(state: Arc<AppState>) {
    let s = state.clone();
    state.runtime.spawn(async move { heartbeat::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { event_sync::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { policy_sync::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { update_check::run(s).await });
    let s = state.clone();
    state.runtime.spawn(async move { attendance_sync::run(s).await });
}
