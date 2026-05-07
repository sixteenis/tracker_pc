//! ============================================================================
//! sync::attendance_sync — 5분 주기 출근 상태 폴링 (기획서 §10).
//! ============================================================================
//!
//! PC 앱은 출근/퇴근 버튼을 제공하지 않고, 스마트폰 앱이 변경한 출근 상태를
//! `GET /api/pc-agent/attendance-status` 로 5분마다 조회한다. `idle_detector`
//! 가 이 상태를 보고 감지 ON/OFF 결정.
//!
//! TODO(2차): 5분은 너무 김 — 출근/퇴근 직후 5분간 PC 앱이 잘못된 상태로 동작할 수
//! 있음. heartbeat 응답에 attendance_status 를 같이 실어주면 3분으로 단축 가능.
//! TODO(서버 연동): 연차/출장/외근 일정도 함께 받아서 시간대별 idle 무시 처리.

use std::sync::Arc;
use std::time::Duration;

use tracing::warn;

use crate::app::AppState;

const POLL_INTERVAL_SECONDS: u64 = 300;

/// 메인 출근 상태 폴링 루프.
pub async fn run(state: Arc<AppState>) {
    // 첫 호출 전 잠깐 대기 (로그인 직후 토큰이 자리잡도록).
    tokio::time::sleep(Duration::from_secs(5)).await;

    loop {
        let maybe_session = state.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => {
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                continue;
            }
        };
        match state.api.get_attendance(&session.access_token).await {
            Ok(snap) => {
                if let Ok(mut s) = state.status.write() {
                    s.attendance = snap.attendance_status;
                }
            }
            Err(e) => warn!(error = %e, "출근 상태 조회 실패"),
        }
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
    }
}
