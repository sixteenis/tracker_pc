//! 백그라운드 동기화 작업 — heartbeat / 이벤트 배치 / 정책 / 업데이트 / 출근 상태.

pub mod attendance_sync;
pub mod event_sync;
pub mod heartbeat;
pub mod policy_sync;
pub mod update_check;

use std::sync::Arc;

use crate::app::AppState;

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
