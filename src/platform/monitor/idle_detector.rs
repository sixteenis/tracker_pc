//! ============================================================================
//! monitor::idle_detector — 자리비움 구간 자동 생성/종료 (기획서 §8, §14).
//! ============================================================================
//!
//! 상태머신:
//!   - Active : 입력이 있는 상태. idle_seconds < threshold.
//!   - IdleOpen { segment_id } : 자리비움 구간 진행 중.
//!
//! 5초마다 `input::idle_seconds()` 를 호출 → 임계값(`effective_idle_threshold_seconds`)
//! 초과 시 segment open. 사용자가 다시 입력하면 close + `IDLE_ENDED` 이벤트.
//!
//! segment 시작 시각은 `now - idle_seconds` 로 보정한다 (마지막 입력 시점이 진짜
//! 자리비움 시작).
//!
//! ── 차단 조건 (기획서 §7, §10) ─────────────────────────────────────────
//! - `can_track_time = false` (요금제 미포함) → 감지 skip
//! - `attendance ∈ {BeforeWork, AfterWork, Outing, Leave, BusinessTrip}` → skip
//!   (출근 전/외출/연차 등은 PC 미사용이 정상)
//! - `attendance = Unknown` 은 안전 기본값으로 감지 진행
//!
//! TODO(기능 미완): lunch 윈도우 분류 (`lunch::classify`) 통합. 현재 segment 가
//! 점심 시간대 안에 있어도 일반 자리비움으로 처리됨. 점심 후보로 분리해서
//! 자동 LUNCH_BREAK 처리 (또는 사용자에게 "점심이었나요?" 토스트) 필요.
//! TODO(2차): 입력이 다시 들어왔을 때 즉시 segment close 하지 않고 grace period
//! (예: 30초) 두기 — 잠깐 마우스 흔들고 다시 자리 비우는 패턴 무시.
//! TODO(2차): 잠금 상태에서 입력이 발생할 수 없음. 현재는 `is_locked` 와 무관하게
//! 동작 — session_events 통합 시 잠금 상태에서는 PC_LOCKED segment 만 생성하도록.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::info;

use crate::data::dto::AttendanceStatus;
use crate::app::{AppState, PcStatus};
use crate::data::local::events_repo;
use crate::data::local::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::platform::monitor::input;

enum IdleState {
    Active,
    IdleOpen { segment_id: String },
}

/// 메인 감지 루프. 앱 종료까지 무한 반복.
pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(state.config.intervals.idle_check_interval_seconds.max(1));
    let mut s = IdleState::Active;
    // 토스트에 표시할 자리비움 시작 시각 — segment open 시 기록, close 시 사용 후 클리어.
    let mut segment_started_at: Option<chrono::DateTime<Utc>> = None;
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
                        segment_started_at = Some(started);
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

                    // 자리비움이 의미 있는 길이로 끝났을 때 토스트로 알림 (백그라운드 thread).
                    // 사용자가 창을 숨겨놓고 일했더라도 트레이/알림센터로 안내됨.
                    if let Some(seg_started) = segment_started_at {
                        let mins = (now - seg_started).num_minutes().max(0);
                        if mins >= 1 {
                            crate::platform::notify::show_explanation_request_async(
                                crate::util::format_local_time(&seg_started),
                                crate::util::format_local_time(&now),
                                mins,
                            );
                        }
                    }
                    segment_started_at = None;
                    s = IdleState::Active;
                } else {
                    info!(segment_id, idle, "자리비움 진행 중");
                }
            }
        }
    }
}

/// 자리비움 segment 한 건 생성 + `IDLE_STARTED` 이벤트 enqueue.
/// 세션이 없으면 `None` (방어 코드 — can_track_time 가드 뒤이므로 사실상 없어야 함).
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
        company_id: session.company_id_str.clone(),
        employee_id: session.employee_id_str.clone(),
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

/// 의미 이벤트를 `local_events` 큐에 추가. 실패 시 warn 로그만 (UI 차단 안 함).
/// 실제 서버 전송은 `sync::event_sync` 가 1분 주기로 처리.
pub(crate) fn enqueue_event(state: &Arc<AppState>, event_type: &str, payload: serde_json::Value) {
    if let Err(e) = events_repo::enqueue(&state.db, event_type, Utc::now(), &payload) {
        tracing::warn!(error = %e, event_type, "이벤트 enqueue 실패");
    }
}
