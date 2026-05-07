//! PC 잠금/잠금해제 감지 (기획서 §10).
//!
//! 1차 MVP 에서는 별도 윈도우 메시지 루프를 만들지 않고, 폴링 기반 근사로
//! 처리한다. Windows 의 `WTSGetActiveConsoleSessionId` + 입력 소켓 핸들 검사
//! 를 사용할 수도 있으나, 외부 의존성을 늘리지 않기 위해 다음 휴리스틱을 사용:
//!
//!   - `idle_detector` 가 보고하는 idle_seconds 가 매우 빨리 0 으로 떨어지지
//!     않는데도 사용자가 입력을 했다는 다른 신호가 없으면 그대로 PC_IDLE 로 처리.
//!   - 본격적인 잠금/잠금해제 이벤트는 2차에서 `WTSRegisterSessionNotification`
//!     + 메시지 윈도우로 정확히 잡는다.
//!
//! 이 stub 은 향후 hook 지점을 명시하기 위해 남겨둔다 — 호출자는 외부에서
//! `record_locked()` / `record_unlocked()` 를 호출하면 된다.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::app::{AppState, PcStatus};
use crate::monitor::idle_detector::enqueue_event;

pub async fn run(state: Arc<AppState>) {
    // 단순 watchdog — 5분마다 살아있음을 로깅.
    let mut tick = tokio::time::interval(Duration::from_secs(300));
    tick.tick().await;
    loop {
        tick.tick().await;
        tracing::trace!("session watchdog tick");
        let _ = &state; // keep arc alive
    }
}

#[allow(dead_code)]
pub fn record_locked(state: &Arc<AppState>) {
    if let Ok(mut s) = state.status.write() {
        s.is_locked = true;
        s.pc_status = PcStatus::Locked;
    }
    enqueue_event(
        state,
        "PC_LOCKED",
        serde_json::json!({ "at": Utc::now().to_rfc3339() }),
    );
}

#[allow(dead_code)]
pub fn record_unlocked(state: &Arc<AppState>) {
    if let Ok(mut s) = state.status.write() {
        s.is_locked = false;
        s.pc_status = PcStatus::Active;
    }
    enqueue_event(
        state,
        "PC_UNLOCKED",
        serde_json::json!({ "at": Utc::now().to_rfc3339() }),
    );
}
