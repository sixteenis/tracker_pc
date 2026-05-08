//! ============================================================================
//! sync::policy_sync — 30분 주기 정책 재조회 (기획서 §22).
//! ============================================================================
//!
//! 관리자가 회사/팀/근로자 자리비움 기준을 변경했을 수 있으므로 주기적으로
//! `GET /api/pc-agent/policy` 호출. 응답의 `policy_version` 이 바뀌면
//! AppState 의 정책 + 라이브 상태(threshold/scope) 즉시 갱신.
//!
//! TODO(2차): heartbeat 응답에 policy_version 가 포함돼 변경 감지를 더 빨리할 수 있음.
//! 현재는 최대 30분 지연 발생 가능.
//! TODO(2차): 정책 변경 시 사용자에게 토스트 안내 ("자리비움 기준이 10분 → 15분으로
//! 변경되었습니다") — 변경 사실을 모르면 사용자가 혼란스러울 수 있음.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::app::AppState;

/// 메인 정책 동기화 루프.
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

        match state.api.get_policy().await {
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
                    s.can_track_time = session.can_track_time() && p.can_track_time;
                    s.last_policy_sync_at = Some(Utc::now());
                }
            }
            Err(e) => warn!(error = %e, "정책 조회 실패"),
        }
    }
}
