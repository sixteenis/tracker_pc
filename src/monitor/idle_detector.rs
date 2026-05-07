//! 5초마다 idle 시간을 측정해서 자리비움 구간을 만든다 (기획서 §8, §14).
//!
//! 상태머신:
//! - Active : 입력이 있는 상태. idle_seconds < threshold.
//! - IdleCandidate : threshold 도달 직전 — 아직 segment open 안함.
//! - IdleOpen(segment_id, started_at) : 자리비움 구간 진행 중.
//!
//! 임계값 도달 시점에 시작 시각은 (now - threshold) 로 보정.
//! 입력이 다시 들어오면 segment 를 close 하고 IDLE_ENDED 이벤트 enqueue.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::info;

use crate::api::types::AttendanceStatus;
use crate::app::{AppState, PcStatus};
use crate::db::events_repo;
use crate::db::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::monitor::input;

enum IdleState {
    Active,
    IdleOpen { segment_id: String },
}

pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(state.config.intervals.idle_check_interval_seconds.max(1));
    let mut s = IdleState::Active;
    info!(check_interval_seconds = ?interval, "idle 감지 루프 시작");

    loop {
        tokio::time::sleep(interval).await;

        let idle = input::idle_seconds();
        let now = Utc::now();

        // status 갱신
        if let Ok(mut st) = state.status.write() {
            st.idle_seconds = idle;
            if idle == 0 {
                st.last_activity_at = now;
            }
        }

        // 추적 권한이 없거나 출근 중이 아니면 idle 구간을 만들지 않는다.
        let (can_track, attendance, threshold, scope) = {
            let st = state.status.read().unwrap();
            (
                st.can_track_time,
                st.attendance,
                st.effective_idle_threshold_seconds,
                st.policy_scope.clone(),
            )
        };

        // 매 사이클마다 한 줄 — 사용자가 동작 여부를 즉시 확인 가능.
        let in_segment = matches!(s, IdleState::IdleOpen { .. });
        info!(
            idle_seconds = idle,
            threshold,
            scope = %scope,
            attendance = ?attendance,
            can_track,
            in_segment,
            "idle 점검"
        );

        if !can_track {
            continue;
        }
        if !attendance.enables_tracking() && attendance != AttendanceStatus::Unknown {
            // 출근 전/외출/연차 등은 PC 미사용이 정상.
            continue;
        }

        match &s {
            IdleState::Active => {
                if idle >= threshold {
                    // segment 시작 시각은 마지막 입력 시점 (= now - idle).
                    let started = now - chrono::Duration::seconds(idle as i64);
                    if let Some(segment_id) = open_segment(&state, started, threshold, &scope) {
                        if let Ok(mut st) = state.status.write() {
                            st.pc_status = PcStatus::Idle;
                        }
                        info!(threshold, scope, "자리비움 구간 시작");
                        s = IdleState::IdleOpen { segment_id };
                    }
                }
            }
            IdleState::IdleOpen { segment_id } => {
                if idle == 0 {
                    // 사용자가 돌아옴 — segment close.
                    let _ = idle_segments_repo::close(&state.db, segment_id, now);
                    enqueue_event(
                        &state,
                        "IDLE_ENDED",
                        serde_json::json!({
                            "segment_id": segment_id,
                            "ended_at": now.to_rfc3339(),
                        }),
                    );
                    if let Ok(mut st) = state.status.write() {
                        st.pc_status = PcStatus::Active;
                    }
                    info!(segment_id, "자리비움 구간 종료");
                    s = IdleState::Active;
                } else {
                    info!(segment_id, idle, "자리비움 진행 중");
                }
            }
        }
    }
}

fn open_segment(
    state: &Arc<AppState>,
    started: chrono::DateTime<Utc>,
    threshold: u64,
    scope: &str,
) -> Option<String> {
    let session = state.session.read().unwrap().clone()?;
    let policy = state.snapshot_policy();
    let deadline = Utc::now()
        + chrono::Duration::hours(policy.explanation_deadline_hours.max(1) as i64);

    let new_seg = NewSegment {
        company_id: session.company_id.clone(),
        employee_id: session.employee_id.clone(),
        device_id: state.device.device_id.clone(),
        work_date: started.date_naive(),
        segment_type: SegmentType::PcIdle,
        start_time: started,
        end_time: None,
        applied_idle_threshold_seconds: threshold as i64,
        policy_scope: scope.to_string(),
        explanation_deadline: Some(deadline),
    };

    let segment_id = match idle_segments_repo::insert(&state.db, &new_seg) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(error = %e, "idle segment 저장 실패");
            return None;
        }
    };

    enqueue_event(
        state,
        "IDLE_STARTED",
        serde_json::json!({
            "segment_id": segment_id,
            "started_at": started.to_rfc3339(),
            "applied_idle_threshold_seconds": threshold,
            "policy_scope": scope,
        }),
    );
    Some(segment_id)
}

pub(crate) fn enqueue_event(state: &Arc<AppState>, event_type: &str, payload: serde_json::Value) {
    if let Err(e) = events_repo::enqueue(&state.db, event_type, Utc::now(), &payload) {
        tracing::warn!(error = %e, event_type, "이벤트 enqueue 실패");
    }
}
