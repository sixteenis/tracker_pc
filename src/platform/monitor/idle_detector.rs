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
//! - `subscription_service::pin_plus_active() == false` (회사 요금제 미포함)
//!   → 마우스/키보드 입력 감지 자체를 호출하지 않음 (`input::idle_seconds()` skip).
//!   요금제 부재 시 "기능 자체가 동작하지 않게" 한다는 기획.
//! - `can_track_time = false` (요금제 미포함 외에도 정책 기반 차단 포함)
//!   → 감지는 하되 segment 생성 skip
//! - `attendance ∈ {BeforeWork, AfterWork, Outing, Leave, BusinessTrip}` → skip
//!   (출근 전/외출/연차 등은 PC 미사용이 정상)
//! - `attendance = Unknown` 은 안전 기본값으로 감지 진행
//! - **`work_status_service::current_status() != WorkingNow` → skip** (2026-05-12 갱신).
//!   V1 `/android/u/get_workstatus.jsp` `result` + `main_info.starttm/endtm` 통합 판별.
//!   미출근 / 퇴근 / Unknown 모두 segment 만들지 않음. UI 라벨과 엔진 게이트가
//!   동일한 진실 소스를 사용해 일관성 보장 (사용자 결정 2026-05-12).
//!
//! ── 점심 시간 처리 (2026-05-13, MVP) ─────────────────────────────────────
//! V1 `get_main2.jsp.brkTime` (도메인: `MainInfo.use_break_time`) 가 true 인 회사는
//! **로컬 12:00~13:00 자리비움을 자동으로 점심으로 간주** — segment 자체 생성 안 함.
//!   - 게이트 1: `now` 가 12:00~13:00 윈도우 안이면 open 시도 skip
//!   - 게이트 2: 12:00 이전 시작 idle 이 13:00 넘어가서 open 되는 경우 — `started` 를
//!     13:00 으로 보정해 점심 시간을 segment 에서 제외
//! `use_break_time=false` 회사는 게이트 없음 (기존 동작 그대로).
//! 추후 회사별 윈도우/허용시간 커스텀 정책으로 확장 예정 — 현 MVP 는 고정 1시간.
//! TODO(2차): 입력이 다시 들어왔을 때 즉시 segment close 하지 않고 grace period
//! (예: 30초) 두기 — 잠깐 마우스 흔들고 다시 자리 비우는 패턴 무시.
//! TODO(2차): 잠금 상태에서 입력이 발생할 수 없음. 현재는 `is_locked` 와 무관하게
//! 동작 — session_events 통합 시 잠금 상태에서는 PC_LOCKED segment 만 생성하도록.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, Local, TimeZone, Timelike, Utc};
use tracing::info;

use crate::data::dto::AttendanceStatus;
use crate::app::{AppState, PcStatus};
use crate::data::local::events_repo;
use crate::data::local::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::domain::service::{main_info_service, subscription_service};
use crate::platform::monitor::input;

/// 점심 시간 윈도우 — 고정 12:00 ~ 13:00 로컬 (MVP).
/// `MainInfo.use_break_time == true` 회사에만 적용. 추후 회사별 커스텀 정책 도입 예정.
const LUNCH_BREAK_START_HOUR: u32 = 12;
const LUNCH_BREAK_END_HOUR: u32 = 13;

enum IdleState {
    Active,
    IdleOpen {
        segment_id: String,
        /// 처음 idle=0 으로 떨어진 시각. 즉시 close 하지 않고 grace 동안 모니터링.
        /// 다시 idle 누적되면 None 으로 복귀(close 취소).
        first_zero_at: Option<chrono::DateTime<Utc>>,
    },
}

/// 입력 복귀(idle=0) 후 진짜 close 까지 대기 시간 (초).
/// 30초 이상 입력이 유지되어야 segment 종료 확정. macOS ioreg 의 일시적 0 반환
/// 또는 OS 짧은 깨어남 이벤트로 segment 가 쪼개지는 사고를 흡수한다.
/// (2026-05-14: "24분 자리비움이 여러 row 로 쪼개짐" 사용자 보고 대응)
const IDLE_CLOSE_GRACE_SECONDS: i64 = 30;

/// 메인 감지 루프. 앱 종료까지 무한 반복.
pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(state.config.intervals.idle_check_interval_seconds.max(1));
    let mut s = IdleState::Active;
    // 토스트에 표시할 자리비움 시작 시각 — segment open 시 기록, close 시 사용 후 클리어.
    let mut segment_started_at: Option<chrono::DateTime<Utc>> = None;
    info!(check_interval_seconds = ?interval, "idle 감지 루프 시작");

    loop {
        tokio::time::sleep(interval).await;

        // ── PIN+ 미사용 회사 — 마우스/키보드 감지 자체를 차단 ────────────────
        // 기획: 요금제 false 면 "기능 자체가 동작하지 않도록". 입력 폴링(input::idle_seconds)
        // 호출 전에 빠르게 끊는다. subscription 응답이 아직 없으면(Option=None) 보수적으로
        // 통과시키지 않고 다음 사이클 대기 — 로그인 흐름은 수 초 내 채워진다.
        if !subscription_service::pin_plus_active() {
            tracing::debug!("[gate ❌ pin_plus_active=false] 폴링 skip (요금제 미사용 또는 미수신)");
            continue;
        }

        let idle = input::idle_seconds();
        let now = Utc::now();
        tracing::debug!(idle, "[input] OS 측정값");

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
            tracing::debug!("[gate ❌ can_track_time=false] segment 생성 skip (요금제·정책 차단)");
            continue;
        }
        if !attendance.enables_tracking() && attendance != AttendanceStatus::Unknown {
            // 출근 전/외출/연차 등은 PC 미사용이 정상.
            tracing::debug!(?attendance, "[gate ❌ attendance] segment 생성 skip");
            continue;
        }

        // V1 `/android/u/get_workstatus.jsp` 기반 통합 판별 (2026-05-12 도입).
        // `WorkingNow` (result>0) 일 때만 segment 생성. 미출근/퇴근/Unknown 은 차단.
        // UI 출근 카드 라벨과 동일한 진실 소스를 사용 → 사용자 체감 일관.
        use crate::domain::service::work_status_service::{self, WorkStatus};
        let work_status = work_status_service::current_status();
        if !work_status.allows_tracking() {
            // WorkingNow 외에는 모두 차단 (NotIn / OffWork / Unknown).
            // Unknown 은 보수적으로 차단 — 정보 부족 시 segment 생성 안 함.
            let _ = WorkStatus::WorkingNow; // unused import 방지용 참조
            tracing::debug!(?work_status, "[gate ❌ work_status≠WorkingNow] segment 생성 skip");
            continue;
        }
        tracing::trace!("[gate ✅ 모든 통과] segment 동작 가능");

        match &s {
            IdleState::Active => {
                if idle >= threshold {
                    // 점심 시간대 (12:00~13:00 로컬, use_break_time=true 회사) — segment open skip.
                    if is_within_lunch_break(now) {
                        info!(idle, threshold, "[gate ❌ lunch_window] segment 생성 skip (점심 시간대)");
                        continue;
                    }
                    // segment 시작 시각은 마지막 입력 시점 (= now - idle).
                    // 점심 시간대를 가로지른 경우 started 를 13:00 로 보정해 점심을 segment 에서 제외.
                    let raw_started = now - chrono::Duration::seconds(idle as i64);
                    let started = snap_started_after_lunch(raw_started);
                    info!(
                        idle, threshold, scope = %scope,
                        started = %started.to_rfc3339(),
                        "[state ⏵ Active→IdleOpen] 임계치 도달 — segment OPEN 시도"
                    );
                    if let Some(segment_id) = open_segment(&state, started, threshold, &scope) {
                        if let Ok(mut st) = state.status.write() {
                            st.pc_status = PcStatus::Idle;
                        }
                        info!(
                            segment_id = %segment_id,
                            started = %started.to_rfc3339(),
                            threshold, scope = %scope,
                            "✅ 자리비움 구간 시작 — 로컬 INSERT (서버 송신은 close 시점에)"
                        );
                        segment_started_at = Some(started);
                        s = IdleState::IdleOpen { segment_id, first_zero_at: None };
                    } else {
                        tracing::warn!("[state ❌] segment open 실패 (세션 없음 또는 DB 오류)");
                    }
                } else {
                    tracing::trace!(idle, threshold, "[state Active] 임계치 미달");
                }
            }
            IdleState::IdleOpen { segment_id, first_zero_at } => {
                let segment_id = segment_id.clone();
                let first_zero_at = *first_zero_at;

                if idle > 0 {
                    // 자리비움 계속 진행 — 직전 idle=0 fallback 있었으면 close 취소.
                    if first_zero_at.is_some() {
                        info!(
                            segment_id = %segment_id, idle,
                            "↩️ [state IdleOpen] grace 중 idle 재누적 — close 취소 (같은 segment 유지)"
                        );
                        s = IdleState::IdleOpen { segment_id, first_zero_at: None };
                    } else {
                        tracing::debug!(segment_id = %segment_id, idle, "[state IdleOpen] 자리비움 진행 중");
                    }
                    continue;
                }

                // idle == 0 — 입력 복귀 후보. grace 미경과면 대기.
                match first_zero_at {
                    None => {
                        // 입력 복귀 첫 감지 — close 즉시 안 하고 grace 시작.
                        info!(
                            segment_id = %segment_id,
                            grace = IDLE_CLOSE_GRACE_SECONDS,
                            "⏳ [state IdleOpen] 입력 복귀 감지 — grace 대기 시작"
                        );
                        s = IdleState::IdleOpen { segment_id, first_zero_at: Some(now) };
                        continue;
                    }
                    Some(zero_at) => {
                        let elapsed = (now - zero_at).num_seconds();
                        if elapsed < IDLE_CLOSE_GRACE_SECONDS {
                            // 아직 grace 경과 안 됨 — 계속 모니터링.
                            tracing::debug!(
                                segment_id = %segment_id, elapsed,
                                grace = IDLE_CLOSE_GRACE_SECONDS,
                                "[state IdleOpen] grace 대기 중"
                            );
                            continue;
                        }
                        info!(
                            segment_id = %segment_id, elapsed,
                            "✅ [state IdleOpen→close] grace 경과 — close 확정"
                        );
                    }
                }

                {
                    // 사용자가 돌아옴 — segment close (grace 경과 후 확정).
                    // close 시각 = 실제 입력 복귀 시점 (zero_at), now 아님.
                    let close_at = first_zero_at.unwrap_or(now);
                    let started = segment_started_at.unwrap_or(now);
                    let duration_secs = (close_at - started).num_seconds().max(0);
                    info!(
                        segment_id = %segment_id,
                        started = %started.to_rfc3339(),
                        close_at = %close_at.to_rfc3339(),
                        duration_secs,
                        "📝 segment CLOSE — 로컬 UPDATE 시작"
                    );

                    if let Err(e) = idle_segments_repo::close(&state.db, &segment_id, close_at) {
                        tracing::warn!(error = %e, segment_id = %segment_id, "❌ 로컬 segment close 실패");
                    } else {
                        tracing::debug!(segment_id = %segment_id, "로컬 segment close 완료");
                    }

                    let applied_threshold = state
                        .status
                        .read()
                        .map(|st| st.effective_idle_threshold_seconds)
                        .unwrap_or(0);
                    let scope_str = state
                        .status
                        .read()
                        .map(|st| st.policy_scope.clone())
                        .unwrap_or_default();

                    info!(
                        segment_id = %segment_id,
                        "📤 이벤트 enqueue: IDLE_STARTED + IDLE_ENDED (PENDING)"
                    );
                    enqueue_event(
                        &state,
                        "IDLE_STARTED",
                        serde_json::json!({
                            "segment_id": segment_id,
                            "started_at": started.to_rfc3339(),
                            "applied_idle_threshold_seconds": applied_threshold,
                            "policy_scope": scope_str,
                        }),
                    );
                    enqueue_event(
                        &state,
                        "IDLE_ENDED",
                        serde_json::json!({
                            "segment_id": segment_id,
                            "ended_at": close_at.to_rfc3339(),
                        }),
                    );

                    // 즉시 송신 시도. 성공 시 끝, 실패 시 1분 주기 재시도.
                    info!(segment_id = %segment_id, "🚀 flush_now 호출 — 서버 즉시 송신 시도");
                    crate::platform::sync::event_sync::flush_now(state.clone());

                    if let Ok(mut st) = state.status.write() {
                        st.pc_status = PcStatus::Active;
                    }
                    info!(
                        segment_id = %segment_id, duration_secs,
                        "✅ [state IdleOpen→Active] 자리비움 구간 종료 (즉시 송신 spawn 완료)"
                    );

                    // 자리비움이 의미 있는 길이로 끝났을 때 토스트로 알림 (백그라운드 thread).
                    // 사용자가 창을 숨겨놓고 일했더라도 트레이/알림센터로 안내됨.
                    if let Some(seg_started) = segment_started_at {
                        let mins = (now - seg_started).num_minutes().max(0);
                        if mins >= 1 {
                            let tz_offset = state.snapshot_policy().time_zone_offset_minutes;
                            crate::platform::notify::show_explanation_request_async(
                                crate::util::format_company_time(&seg_started, tz_offset),
                                crate::util::format_company_time(&now, tz_offset),
                                mins,
                            );
                        }
                    }
                    segment_started_at = None;
                    s = IdleState::Active;
                }
            }
        }
    }
}

/// 자리비움 segment 한 건 생성. **이벤트 enqueue 는 close 시점에 한 번에 처리** (2026-05-14).
/// 진행 중 segment 가 서버에 INSERT 되지 않도록 변경 — "진행 중·진행 중" 표시 사고 회피
/// + dangling segment (IDLE_STARTED 만 보내고 IDLE_ENDED 못 보냄) 위험 제거.
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
        // work_date 는 사용자 거주 시간대(로컬) 기준이어야 함.
        // UTC 기준이면 KST 새벽~오전 9시 segment 가 전날로 기록되어 "오늘 발생한 소명"
        // 필터에서 누락된다. 사용자 결정 2026-05-13.
        work_date: started.with_timezone(&Local).date_naive(),
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

    // 의도적으로 IDLE_STARTED enqueue 안 함 — close 시점에 한 번에 enqueue.
    Some(segment_id)
}

/// 의미 이벤트를 `local_events` 큐에 추가. 실패 시 warn 로그만 (UI 차단 안 함).
/// 실제 서버 전송은 `sync::event_sync` 가 1분 주기로 처리.
pub(crate) fn enqueue_event(state: &Arc<AppState>, event_type: &str, payload: serde_json::Value) {
    let payload_preview = payload.to_string();
    match events_repo::enqueue(&state.db, event_type, Utc::now(), &payload) {
        Ok(event_id) => tracing::info!(
            event_type, event_id = %event_id, payload = %payload_preview,
            "💾 enqueue OK (local_events PENDING)"
        ),
        Err(e) => tracing::warn!(error = %e, event_type, "❌ enqueue 실패"),
    }
}

/// 로컬 기준 자정 이후 초 수 (`Utc` → `Local` 변환 후 시·분·초 계산).
fn local_seconds_in_day(utc: chrono::DateTime<Utc>) -> u32 {
    let local = utc.with_timezone(&Local);
    local.hour() * 3600 + local.minute() * 60 + local.second()
}

/// `use_break_time=true` 회사에 한해, 로컬 기준 시각이 점심 윈도우 안인지 판정.
/// `use_break_time=false` 회사 또는 `MainInfo` 미수신 상태에서는 항상 false.
fn is_within_lunch_break(utc: chrono::DateTime<Utc>) -> bool {
    let info = match main_info_service::current() {
        Some(m) => m,
        None => return false,
    };
    if !info.use_break_time {
        return false;
    }
    let s = local_seconds_in_day(utc);
    s >= LUNCH_BREAK_START_HOUR * 3600 && s < LUNCH_BREAK_END_HOUR * 3600
}

/// `started` 가 점심 윈도우 안이면 그 날짜의 13:00 로컬 시각으로 보정한다.
/// 12:00 이전 또는 13:00 이후는 그대로 반환. `use_break_time=false` 회사에서는 영향 없음.
fn snap_started_after_lunch(started: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    let info = match main_info_service::current() {
        Some(m) => m,
        None => return started,
    };
    if !info.use_break_time {
        return started;
    }
    let s = local_seconds_in_day(started);
    if s < LUNCH_BREAK_START_HOUR * 3600 || s >= LUNCH_BREAK_END_HOUR * 3600 {
        return started;
    }
    let local = started.with_timezone(&Local);
    Local
        .with_ymd_and_hms(local.year(), local.month(), local.day(), LUNCH_BREAK_END_HOUR, 0, 0)
        .single()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(started)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    fn local_utc(h: u32, m: u32) -> chrono::DateTime<Utc> {
        Local
            .with_ymd_and_hms(2026, 5, 13, h, m, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn snap_keeps_when_started_before_noon() {
        // MainInfo 캐시가 없을 때는 보정 안 함 (테스트 환경은 캐시 없음)
        let t = local_utc(11, 30);
        assert_eq!(snap_started_after_lunch(t), t);
    }

    #[test]
    fn snap_keeps_when_started_after_one() {
        let t = local_utc(13, 30);
        assert_eq!(snap_started_after_lunch(t), t);
    }

    #[test]
    fn lunch_gate_false_when_no_main_info_cache() {
        // 테스트는 main_info_service::CURRENT 가 None 인 상태로 실행.
        // use_break_time 비활성 동일 효과 — 항상 false.
        assert!(!is_within_lunch_break(local_utc(12, 30)));
    }

    #[test]
    fn local_seconds_in_day_basic() {
        let t = local_utc(12, 30); // 12:30:00
        assert_eq!(local_seconds_in_day(t), 12 * 3600 + 30 * 60);
    }
}
