//! 출근 상태 폴링 (기획서 §10).
//!
//! PC 앱은 출근/퇴근 버튼을 제공하지 않고, 스마트폰 앱이 변경한 출근 상태를
//! 5분 주기로 조회해서 idle 감지 ON/OFF 를 결정한다.

use std::sync::Arc;
use std::time::Duration;

use tracing::warn;

use crate::app::AppState;

const POLL_INTERVAL_SECONDS: u64 = 300;

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
