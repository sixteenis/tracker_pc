//! 30분 주기 정책 조회 — 자리비움 기준 시간이 관리자에 의해 변경되었을 수 있으므로
//! 주기적으로 다시 받는다 (기획서 §22).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::app::AppState;

pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(
        state.config.intervals.policy_check_interval_seconds.max(60),
    );
    loop {
        tokio::time::sleep(interval).await;

        let maybe_session = state.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => continue,
        };

        match state.api.get_policy(&session.access_token).await {
            Ok(p) => {
                let prev_version = state.snapshot_policy().policy_version;
                if p.policy_version != prev_version {
                    info!(prev = prev_version, new = p.policy_version, "정책 변경 감지");
                }
                if let Ok(mut policy) = state.policy.write() {
                    *policy = p.clone();
                }
                if let Ok(mut s) = state.status.write() {
                    s.effective_idle_threshold_seconds = p.effective_idle_threshold_seconds;
                    s.policy_scope = p.policy_scope.clone();
                    s.policy_version = p.policy_version;
                    s.can_track_time = session.subscription.can_track_time && p.can_track_time;
                    s.last_policy_sync_at = Some(Utc::now());
                }
            }
            Err(e) => warn!(error = %e, "정책 조회 실패"),
        }
    }
}
