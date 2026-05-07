//! 백그라운드 감지 루프 — 키보드/마우스 비활성, 세션 락/언락, 앱 라이프사이클.
//!
//! 모든 루프는 `AppState` 가 살아있는 동안 유지되며, `state.runtime.spawn` 으로
//! tokio task 로 실행된다. 키/마우스 입력 자체는 절대 서버로 보내지 않는다 —
//! 로컬에서 idle 여부만 판단해서 의미 있는 이벤트만 enqueue 한다.

pub mod idle_detector;
pub mod input;
pub mod lifecycle;
pub mod policy;
pub mod session_events;

use std::sync::Arc;

use crate::app::AppState;

pub fn spawn_all(state: Arc<AppState>) {
    // APP_STARTED 이벤트는 즉시 enqueue.
    let _ = lifecycle::record_started(&state);

    let s1 = state.clone();
    state.runtime.spawn(async move {
        idle_detector::run(s1).await;
    });

    let s2 = state.clone();
    state.runtime.spawn(async move {
        session_events::run(s2).await;
    });
}
