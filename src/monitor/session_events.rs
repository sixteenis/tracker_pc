//! ============================================================================
//! monitor::session_events — PC 잠금/잠금해제 감지 (기획서 §10).
//! ============================================================================
//!
//! ⚠️ 1차 MVP STUB ⚠️
//!
//! 본격적인 잠금/해제 이벤트 캡처는 미구현. 현재 `run()` 은 5분 watchdog 만 돌고,
//! `record_locked()` / `record_unlocked()` 외부 호출 hook 만 노출.
//!
//! TODO(2차 핵심): Windows `WTSRegisterSessionNotification` + hidden message window
//! 로 `WM_WTSSESSION_CHANGE` 를 받아 `WTS_SESSION_LOCK` / `WTS_SESSION_UNLOCK` 을
//! 감지. 메시지 펌프는 별도 thread + `winapi::PostThreadMessage` 로 종료 가능하게.
//!
//! TODO(2차): macOS `NSDistributedNotificationCenter` 의 "com.apple.screenIsLocked"
//! / "com.apple.screenIsUnlocked" notification 구독.
//!
//! TODO(idle_detector 통합): 잠금 상태에서 입력은 발생할 수 없으므로 PC_LOCKED
//! segment 만 생성해야 하는데, 현재는 `idle_detector` 가 PC_IDLE 로 처리할 수 있음.
//! 두 모듈 통합 (또는 잠금 우선 분기) 필요.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::app::{AppState, PcStatus};
use crate::monitor::idle_detector::enqueue_event;

/// stub — 5분마다 살아있음 로깅. TODO(2차) 에서 실제 잠금 감지로 교체.
pub async fn run(state: Arc<AppState>) {
    let mut tick = tokio::time::interval(Duration::from_secs(300));
    tick.tick().await;
    loop {
        tick.tick().await;
        tracing::trace!("session watchdog tick");
        let _ = &state; // keep arc alive
    }
}

/// 잠금 감지 hook. 2차에서 메시지 윈도우로부터 호출될 예정.
/// 현재는 외부에서 직접 호출할 수 있게 노출만 해둠.
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

/// 잠금 해제 감지 hook. 자세한 내용은 `record_locked` 참고.
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
