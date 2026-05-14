//! ============================================================================
//! sync::event_sync — 1분 주기 이벤트 배치 전송 (기획서 §17, §18, §22).
//! ============================================================================
//!
//! - `local_events` 의 PENDING/FAILED 를 최대 50건씩 (`max_events_per_batch`) 묶어
//!   `POST /api/pc-agent/events` 로 전송.
//! - 응답의 `accepted_event_ids` 만 SUCCESS, 나머지는 FAILED + retry_count++.
//! - 멱등성: 같은 `event_id` 가 두 번 가도 서버는 한 번만 저장.
//!
//! ── PRESENCE / lifecycle 이벤트 송신 보장 (heartbeat 제거 결정 2026-05-12) ──
//! `can_track_time = false` (pinpluse=false 등) 케이스에서도 LOGIN/LOGOUT/
//! PC_SHUTDOWN/APP_STARTED/APP_STOPPED 같은 lifecycle·PRESENCE 이벤트는 감사상
//! 반드시 송신되어야 한다 (서버측 `PCAGT_PRESENCE_LOG` 의 진실 소스). 따라서
//! 본 루프에는 `can_track_time` 게이트를 두지 않는다.
//!   - idle 류 차단은 상위 `monitor::idle_detector` 의 `pin_plus_active()` 가드가
//!     이미 enqueue 자체를 막으므로, 큐에 들어온 이벤트는 송신해도 노이즈 없음.
//!   - 단일 보호 조건은 "세션 존재" 뿐 (로그인 전엔 발신 대상 식별 불가).
//!
//! TODO(2차): retry_count 임계 (예: 20) 초과 시 dead-letter 분리.
//! TODO(2차): 수동 동기화 트리거 — `tokio::sync::mpsc` 로 UI 의 "지금 동기화"
//! 버튼과 연결. 현재는 1분 대기.
//! TODO(2차): 네트워크 끊김 감지 — 연속 N회 실패 시 backoff 늘리고 UI 에 오프라인
//! 뱃지 표시.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::constants;
use crate::data::dto::{EventEntry, EventsBatch, ExplanationSubmit};
use crate::app::AppState;
use crate::data::local::{events_repo, explanations_repo};

/// 이벤트 배치 송신 주기 — **고정 60초**.
/// heartbeat 제거 결정(2026-05-12) 으로 정책 응답·클라 설정 모두에서
/// 이 값이 사라졌으므로 코드 상수로 명시. 변경하려면 본 상수만 수정.
const EVENT_BATCH_INTERVAL_SECONDS: u64 = 60;

/// 메인 배치 전송 루프.
pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(EVENT_BATCH_INTERVAL_SECONDS);

    loop {
        tokio::time::sleep(interval).await;
        flush_once(&state).await;

        // 같은 주기에 소명 제출 재시도 — 즉시 제출 실패 / 네트워크 끊김으로 로컬에
        // 남은 PENDING/FAILED row 를 한 건씩 재전송. 성공 시 로컬 row 물리 삭제.
        let maybe_session = state.session.read().unwrap().clone();
        if let Some(session) = maybe_session {
            retry_pending_explanations(&state, &session).await;
        }
    }
}

/// 외부 호출용 — 자리비움 종료(close) 같은 즉시 송신이 필요한 시점에 spawn 해서 사용.
/// `local_events` PENDING/FAILED 가 있으면 한 사이클을 비동기로 한 번 돌린다.
/// 1분 루프와 별도로 동작 — 중복 호출돼도 멱등 (서버가 event_id UNIQUE).
pub fn flush_now(state: Arc<AppState>) {
    state.runtime.clone().spawn(async move {
        flush_once(&state).await;
    });
}

/// 한 사이클의 송신 본문 (run + flush_now 공통).
async fn flush_once(state: &Arc<AppState>) {
    let limit = state.config.intervals.max_events_per_batch.max(1);

    let maybe_session = state.session.read().unwrap().clone();
    let session = match maybe_session {
        Some(s) => s,
        None => return,
    };

    let pending = match events_repo::pending_batch(&state.db, limit) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "PENDING 이벤트 조회 실패");
            return;
        }
    };
    if pending.is_empty() {
        return;
    }

    let event_ids: Vec<String> = pending.iter().map(|e| e.event_id.clone()).collect();
    let entries: Vec<EventEntry> = pending
        .into_iter()
        .map(|e| EventEntry {
            event_id: e.event_id,
            event_type: e.event_type,
            event_time: e.event_time,
            payload: serde_json::from_str(&e.payload_json)
                .unwrap_or(serde_json::Value::Null),
        })
        .collect();

    let has_segment_event = entries.iter().any(|e| {
        matches!(e.event_type.as_str(), "IDLE_STARTED" | "IDLE_ENDED" | "NO_PC_RECORD")
    });

    let batch = EventsBatch {
        company_id: session.company_id_str.clone(),
        employee_id: session.employee_id_str.clone(),
        device_id: state.device.device_id.clone(),
        events: entries,
    };

    match state.api.send_events(batch).await {
        Ok(resp) => {
            let count = resp.accepted_event_ids.len();
            let accepted_any = !resp.accepted_event_ids.is_empty();
            let _ = events_repo::mark_success(&state.db, &resp.accepted_event_ids);
            if let Ok(mut s) = state.status.write() {
                s.last_event_sync_at = Some(Utc::now());
            }
            info!(count, "이벤트 배치 전송 성공");

            if has_segment_event && accepted_any {
                crate::ui::explanation_list_view::request_refresh(
                    state.clone(),
                    session.employee_id,
                );
            }
        }
        Err(e) => {
            warn!(error = %e, count = event_ids.len(), "이벤트 배치 전송 실패 — 재시도 대기");
            let _ = events_repo::mark_failed(&state.db, &event_ids, &e.to_string());
        }
    }
}

/// `explanations.sync_status IN ('PENDING','FAILED')` 를 한 주기당 처리.
/// 한 건씩 순차 재전송 — 한 건 실패해도 다음 건 계속 시도. 성공 시 `delete`.
async fn retry_pending_explanations(
    state: &Arc<AppState>,
    session: &crate::domain::model::user::User,
) {
    let pending = match explanations_repo::pending_batch(&state.db, 20) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "explanations PENDING 조회 실패");
            return;
        }
    };
    if pending.is_empty() {
        return;
    }
    let device_id = state.device.device_id.clone();
    let company_id = session.company_id_str.clone();
    let employee_id = session.employee_id_str.clone();

    for row in pending {
        let payload = ExplanationSubmit {
            employee_id: employee_id.clone(),
            company_id: company_id.clone(),
            device_id: device_id.clone(),
            segment_id: row.segment_id.clone(),
            explanation_type: row.explanation_type.clone(),
            explanation_text: row.explanation_text.clone(),
            other_type_label: row.other_type_label.clone(),
            submitted_from: constants::SUBMITTED_FROM_PC_APP.to_string(),
            // segment 메타 — 서버에 segment 가 아직 없으면 upsert 용. 재시도 시점에
            // 정확한 메타가 필요하므로 explanations 테이블에 같이 보관된 값을 사용.
            work_date: row.work_date.format("%Y-%m-%d").to_string(),
            segment_type: "PC_IDLE".to_string(),
            start_time: row.start_time,
            end_time: Some(row.end_time),
            duration_seconds: Some(row.duration_seconds),
            applied_idle_threshold_seconds: 0,
            policy_scope: "DEFAULT".to_string(),
        };
        match state.api.submit_explanation(payload).await {
            Ok(()) => {
                if let Err(e) = explanations_repo::delete(&state.db, row.id) {
                    warn!(error = %e, id = row.id, "explanation 로컬 삭제 실패");
                } else {
                    info!(id = row.id, segment_id = %row.segment_id, "explanation 재시도 성공 → 로컬 삭제");
                }
            }
            Err(e) => {
                // 영구 거부(재시도 무의미) vs 일시 에러(재시도 의미 있음) 구분.
                // 영구 거부:
                //   - `400 INVALID_EXPLANATION_TYPE` — 회사 룩업에 없는 코드 (회사가 사유 비활성화/변경)
                //   - `400 INVALID_*` 류 — 입력 검증 실패 (재시도해도 동일 결과)
                //   - `403 FORBIDDEN` — 다른 EMPSID segment 에 소명 시도
                //   - `404` — segment 자체가 서버에 없음 (이미 정리됨)
                // 영구 거부는 매분 노이즈만 만들므로 즉시 로컬 폐기.
                let msg = e.to_string().to_ascii_uppercase();
                let permanent = msg.contains("INVALID_EXPLANATION_TYPE")
                    || msg.contains("HTTP 400")
                    || msg.contains("HTTP 403")
                    || msg.contains("HTTP 404");
                if permanent {
                    let _ = explanations_repo::delete(&state.db, row.id);
                    warn!(
                        error = %e,
                        id = row.id,
                        segment_id = %row.segment_id,
                        "explanation 영구 거부 — 로컬 폐기 (재시도 중단)"
                    );
                } else {
                    let _ = explanations_repo::mark_failed(&state.db, row.id, &e.to_string());
                    warn!(error = %e, id = row.id, "explanation 재시도 실패 — 다음 주기 대기");
                }
                // 한 건 실패/폐기해도 다음 건 계속 시도.
            }
        }
    }
}
