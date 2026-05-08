//! ============================================================================
//! monitor — 로컬 PC 상태 감지 모듈 (idle / 잠금 / 앱 라이프사이클).
//! ============================================================================
//!
//! 모든 루프는 `AppState` 가 살아있는 동안 유지되며, `state.runtime.spawn` 으로
//! tokio task 로 실행된다.
//!
//! ── 보안/개인정보 (기획서 §19, §23) ─────────────────────────────────────
//! - 키/마우스 입력 자체는 절대 서버로 보내지 않는다.
//! - 로컬에서 idle 여부 (사용/미사용) 만 판단해서 의미 있는 이벤트만 enqueue.
//! - 화면/URL/프로그램명 등은 수집하지 않는다.
//!
//! 서브모듈:
//!   - input          : OS 별 idle 시간 측정 (Windows GetLastInputInfo / macOS ioreg)
//!   - idle_detector  : 5초 폴링 → 자리비움 segment 생성/종료
//!   - session_events : PC 잠금/해제 (1차 stub, 2차 WTSRegisterSessionNotification)
//!   - lifecycle      : APP_STARTED / NO_PC_RECORD 자동 검출
//!   - policy         : employee → team → company → default 우선순위 해석

pub mod idle_detector;
pub mod input;
pub mod lifecycle;
pub mod policy;
pub mod session_events;

use std::sync::Arc;

use crate::app::AppState;

/// 앱 시작 시 한 번 호출 — 모든 감지 루프를 백그라운드 task 로 띄운다.
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
