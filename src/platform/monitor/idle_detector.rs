//! ============================================================================
//! monitor::idle_detector — 자리비움 구간 자동 생성/종료 (기획서 §8, §14).
//! ============================================================================
//!
//! 상태머신:
//!   - Active : 입력이 있는 상태. idle_seconds < threshold 또는 점심 누적 중.
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
//!   → 마우스/키보드 입력 감지 자체를 호출하지 않음.
//! - `can_track_time = false` → segment 생성 skip
//! - `attendance ∈ {BeforeWork, AfterWork, Outing, Leave, BusinessTrip}` → skip
//! - `work_status_service::current_status() != WorkingNow` → skip
//!
//! ── 점심 시간 처리 Phase 2 (2026-05-14) ──────────────────────────────────
//! 회사 정책의 `lunch_start_time`/`lunch_end_time`/`lunch_allowed_minutes` 를 사용한
//! **누적 추적** 모델. `MainInfo.use_break_time=true` 회사에만 적용.
//!
//! 모델 (사용자 결정 2026-05-14, 안 2):
//!   - 윈도우: `lunch_start_time` ~ `lunch_end_time` (회사 timezone)
//!   - 허용: `lunch_allowed_minutes` (분)
//!   - 하루 동안 윈도우 안에서 idle 누적량을 추적
//!   - 누적 < 허용: 점심으로 인정 → segment 생성 안 함
//!   - 누적 ≥ 허용: 한도 초과 → segment OPEN (일반 처리)
//!   - 자정 자동 리셋
//!
//! 예시: 윈도우 12:30~14:30, 허용 60분
//!   - 12:30~13:30 (60분 idle): 점심 인정, segment 없음
//!   - 13:30~14:30 (윈도우 안이지만 누적 60분 초과): segment OPEN, started=13:30
//!   - 14:30+ (윈도우 밖): 일반 segment 동작
//!
//! IdleOpen 상태에서 윈도우 진입: 점심 시작 시각(window_start) 으로 close 후 Active 복귀.
//! 이후 윈도우 안 누적 추적으로 전환.
//!
//! TODO(2차): 잠금 상태에서 입력이 발생할 수 없음 — session_events 통합 시 PC_LOCKED segment 처리.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Timelike, Utc};
use tracing::info;

use crate::data::dto::AttendanceStatus;
use crate::app::{AppState, PcStatus};
use crate::data::local::events_repo;
use crate::data::local::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::domain::service::{main_info_service, subscription_service};
use crate::platform::monitor::input;

enum IdleState {
    Active,
    IdleOpen {
        segment_id: String,
        /// 처음 idle=0 으로 떨어진 시각. 즉시 close 하지 않고 grace 동안 모니터링.
        /// 다시 idle 누적되면 None 으로 복귀(close 취소).
        first_zero_at: Option<DateTime<Utc>>,
    },
}

/// 입력 복귀(idle=0) 후 진짜 close 까지 대기 시간 (초).
/// 30초 이상 입력이 유지되어야 segment 종료 확정. macOS ioreg 의 일시적 0 반환
/// 또는 OS 짧은 깨어남 이벤트로 segment 가 쪼개지는 사고를 흡수한다.
/// (2026-05-14: "24분 자리비움이 여러 row 로 쪼개짐" 사용자 보고 대응)
const IDLE_CLOSE_GRACE_SECONDS: i64 = 30;

/// 메인 감지 루프. 앱 종료까지 무한 반복.
pub async fn run(state: Arc<AppState>) {
    let interval_secs = state.config.intervals.idle_check_interval_seconds.max(1);
    let interval = Duration::from_secs(interval_secs);
    let mut s = IdleState::Active;
    // 토스트에 표시할 자리비움 시작 시각 — segment open 시 기록, close 시 사용 후 클리어.
    let mut segment_started_at: Option<DateTime<Utc>> = None;
    // 점심 시간 누적 추적 (회사 timezone 기준 오늘 윈도우 안 idle 누적 초).
    let mut lunch_used_secs: u64 = 0;
    let mut lunch_reset_date: Option<NaiveDate> = None;
    info!(check_interval_seconds = ?interval, "idle 감지 루프 시작");

    loop {
        tokio::time::sleep(interval).await;

        // ── PIN+ 미사용 회사 — 마우스/키보드 감지 자체를 차단 ────────────────
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

        // 회사 timezone 기준 오늘 날짜 — 자정 리셋용.
        let tz = state.snapshot_policy().time_zone_offset_minutes;
        let today_company = now
            .with_timezone(&crate::util::company_offset(tz))
            .date_naive();
        if lunch_reset_date != Some(today_company) {
            if lunch_reset_date.is_some() {
                info!(
                    prev = ?lunch_reset_date,
                    today = %today_company,
                    "🌙 자정 통과 — 점심 누적 리셋"
                );
            }
            lunch_used_secs = 0;
            lunch_reset_date = Some(today_company);
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
        let in_lunch_window = is_within_lunch_break(&state, now);
        let lunch_allowed = lunch_allowed_seconds(&state);
        info!(
            idle_seconds = idle,
            threshold,
            scope = %scope,
            attendance = ?attendance,
            can_track,
            in_segment,
            in_lunch_window,
            lunch_used_secs,
            lunch_allowed,
            "idle 점검"
        );

        if !can_track {
            tracing::debug!("[gate ❌ can_track_time=false] segment 생성 skip (요금제·정책 차단)");
            continue;
        }
        if !attendance.enables_tracking() && attendance != AttendanceStatus::Unknown {
            tracing::debug!(?attendance, "[gate ❌ attendance] segment 생성 skip");
            continue;
        }

        use crate::domain::service::work_status_service::{self, WorkStatus};
        let work_status = work_status_service::current_status();
        if !work_status.allows_tracking() {
            let _ = WorkStatus::WorkingNow; // unused import 방지용 참조
            tracing::debug!(?work_status, "[gate ❌ work_status≠WorkingNow] segment 생성 skip");
            continue;
        }
        tracing::trace!("[gate ✅ 모든 통과] segment 동작 가능");

        match &s {
            IdleState::Active => {
                if idle < threshold {
                    tracing::trace!(idle, threshold, "[state Active] 임계치 미달");
                    continue;
                }

                // 점심 윈도우 안 + 누적 < 허용 → 점심으로 인정, segment 안 만듦.
                if in_lunch_window && lunch_used_secs < lunch_allowed {
                    let added = interval_secs.min(lunch_allowed - lunch_used_secs);
                    lunch_used_secs += added;
                    info!(
                        idle, threshold,
                        added,
                        lunch_used_secs, lunch_allowed,
                        "[state Active+lunch] 점심 누적 추적 (segment 없음)"
                    );
                    continue;
                }

                // segment open — 시작 시각 보정.
                // 1) 윈도우 안에서 누적 한도 초과: started = max(raw_started, 한도 도달 시각)
                // 2) 윈도우 밖 + raw_started 가 윈도우 내/이전: started = window_end
                // 3) 그 외: started = raw_started
                let raw_started = now - chrono::Duration::seconds(idle as i64);
                let started = effective_segment_start(&state, raw_started, now, lunch_allowed);
                info!(
                    idle, threshold, scope = %scope,
                    raw_started = %raw_started.to_rfc3339(),
                    started = %started.to_rfc3339(),
                    in_lunch_window, lunch_used_secs, lunch_allowed,
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
            }
            IdleState::IdleOpen { segment_id, first_zero_at } => {
                let segment_id = segment_id.clone();
                let first_zero_at = *first_zero_at;

                // 윈도우 진입 강제 close: segment 가 점심 윈도우 안으로 넘어가면
                // window_start 시점에 close 하고 Active 복귀. 이후 윈도우 안 누적 추적.
                if in_lunch_window {
                    if let Some(window_start_utc) = lunch_window_today_utc(&state, now)
                        .map(|(start, _end)| start)
                    {
                        let started = segment_started_at.unwrap_or(now);
                        let close_at = std::cmp::max(window_start_utc, started);
                        info!(
                            segment_id = %segment_id,
                            window_start = %window_start_utc.to_rfc3339(),
                            close_at = %close_at.to_rfc3339(),
                            "🍱 [state IdleOpen+lunch_window] 윈도우 진입 — segment 강제 close"
                        );
                        finalize_segment_close(
                            &state,
                            &segment_id,
                            started,
                            close_at,
                            segment_started_at,
                            now,
                        );
                        segment_started_at = None;
                        s = IdleState::Active;
                        continue;
                    }
                }

                if idle > 0 {
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

                let close_at = first_zero_at.unwrap_or(now);
                let started = segment_started_at.unwrap_or(now);
                finalize_segment_close(
                    &state,
                    &segment_id,
                    started,
                    close_at,
                    segment_started_at,
                    now,
                );
                segment_started_at = None;
                s = IdleState::Active;
            }
        }
    }
}

/// segment close + 이벤트 enqueue + 토스트 알림 + 즉시 송신 spawn 까지 한 번에 처리.
/// (Active 복귀까지의 부수효과 묶음 — IdleOpen 의 정상 close 와 윈도우 진입 강제 close 가 공유.)
fn finalize_segment_close(
    state: &Arc<AppState>,
    segment_id: &str,
    started: DateTime<Utc>,
    close_at: DateTime<Utc>,
    segment_started_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) {
    let duration_secs = (close_at - started).num_seconds().max(0);
    info!(
        segment_id,
        started = %started.to_rfc3339(),
        close_at = %close_at.to_rfc3339(),
        duration_secs,
        "📝 segment CLOSE — 로컬 UPDATE 시작"
    );

    if let Err(e) = idle_segments_repo::close(&state.db, segment_id, close_at) {
        tracing::warn!(error = %e, segment_id, "❌ 로컬 segment close 실패");
    } else {
        tracing::debug!(segment_id, "로컬 segment close 완료");
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

    info!(segment_id, "📤 이벤트 enqueue: IDLE_STARTED + IDLE_ENDED (PENDING)");
    enqueue_event(
        state,
        "IDLE_STARTED",
        serde_json::json!({
            "segment_id": segment_id,
            "started_at": started.to_rfc3339(),
            "applied_idle_threshold_seconds": applied_threshold,
            "policy_scope": scope_str,
        }),
    );
    enqueue_event(
        state,
        "IDLE_ENDED",
        serde_json::json!({
            "segment_id": segment_id,
            "ended_at": close_at.to_rfc3339(),
        }),
    );

    info!(segment_id, "🚀 flush_now 호출 — 서버 즉시 송신 시도");
    crate::platform::sync::event_sync::flush_now(state.clone());

    if let Ok(mut st) = state.status.write() {
        st.pc_status = PcStatus::Active;
    }
    info!(
        segment_id, duration_secs,
        "✅ [IdleOpen→Active] 자리비움 구간 종료 (즉시 송신 spawn 완료)"
    );

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
}

/// 자리비움 segment 한 건 생성. **이벤트 enqueue 는 close 시점에 한 번에 처리** (2026-05-14).
fn open_segment(
    state: &Arc<AppState>,
    started: DateTime<Utc>,
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
        work_date: started.with_timezone(&Local).date_naive(),
        segment_type: SegmentType::PcIdle,
        start_time: started,
        end_time: None,
        applied_idle_threshold_seconds: threshold as i64,
        policy_scope: scope.to_string(),
        explanation_deadline: Some(deadline),
    };

    match idle_segments_repo::insert(&state.db, &new_seg) {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(error = %e, "idle segment 저장 실패");
            None
        }
    }
}

/// 의미 이벤트를 `local_events` 큐에 추가.
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

// ─────────────────────── 점심 시간 정책 헬퍼 (Phase 2) ───────────────────────

/// "HH:MM" → 자정 이후 초 수. 파싱 실패 시 None.
fn parse_hhmm_to_secs(s: &str) -> Option<u32> {
    let mut parts = s.trim().split(':');
    let h: u32 = parts.next()?.trim().parse().ok()?;
    let m: u32 = parts.next()?.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 3600 + m * 60)
}

/// 회사 timezone 기준 자정 이후 초 수.
fn company_seconds_in_day(utc: DateTime<Utc>, tz_offset_minutes: i32) -> u32 {
    let local = utc.with_timezone(&crate::util::company_offset(tz_offset_minutes));
    local.hour() * 3600 + local.minute() * 60 + local.second()
}

/// 회사 정책의 점심 윈도우 (시작, 종료) — 자정 이후 초 수.
/// `use_break_time=false` 또는 파싱 실패면 None. start ≥ end 면 None.
fn lunch_window_secs(state: &Arc<AppState>) -> Option<(u32, u32)> {
    let info = main_info_service::current()?;
    if !info.use_break_time {
        return None;
    }
    let policy = state.snapshot_policy();
    let start = parse_hhmm_to_secs(&policy.lunch_start_time)?;
    let end = parse_hhmm_to_secs(&policy.lunch_end_time)?;
    if end <= start {
        return None;
    }
    Some((start, end))
}

/// 회사 정책의 점심 허용 시간 (초). `use_break_time=false` 든 무관하게 정책값 반환.
fn lunch_allowed_seconds(state: &Arc<AppState>) -> u64 {
    (state.snapshot_policy().lunch_allowed_minutes as u64) * 60
}

/// 회사 timezone 기준 오늘 날짜의 점심 윈도우 UTC 범위.
fn lunch_window_today_utc(
    state: &Arc<AppState>,
    now: DateTime<Utc>,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let (start_secs, end_secs) = lunch_window_secs(state)?;
    let tz = state.snapshot_policy().time_zone_offset_minutes;
    let offset = crate::util::company_offset(tz);
    let date = now.with_timezone(&offset).date_naive();
    let midnight = offset
        .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
        .single()?;
    let start_local = midnight + chrono::Duration::seconds(start_secs as i64);
    let end_local = midnight + chrono::Duration::seconds(end_secs as i64);
    Some((start_local.with_timezone(&Utc), end_local.with_timezone(&Utc)))
}

/// `now` 가 점심 윈도우 안인지 (회사 timezone 기준, 끝값 exclusive).
fn is_within_lunch_break(state: &Arc<AppState>, now: DateTime<Utc>) -> bool {
    let (start, end) = match lunch_window_secs(state) {
        Some(t) => t,
        None => return false,
    };
    let s = company_seconds_in_day(now, state.snapshot_policy().time_zone_offset_minutes);
    s >= start && s < end
}

/// segment 시작 시각 보정 — Phase 2 누적 모델에 맞춤.
///
/// 케이스:
/// 1) 윈도우 안에서 open (누적 한도 초과): started = max(raw_started, win_start + allowed)
/// 2) 윈도우 밖 + raw_started < win_end: started = win_end (점심을 segment 에서 제외)
/// 3) 그 외: raw_started 그대로
///
/// `use_break_time=false` 또는 윈도우 파싱 실패면 raw_started 그대로.
fn effective_segment_start(
    state: &Arc<AppState>,
    raw_started: DateTime<Utc>,
    now: DateTime<Utc>,
    lunch_allowed: u64,
) -> DateTime<Utc> {
    let (win_start, win_end) = match lunch_window_today_utc(state, now) {
        Some(t) => t,
        None => return raw_started,
    };
    effective_segment_start_at(raw_started, now, win_start, win_end, lunch_allowed)
}

/// 순수 함수 버전 — 테스트 용이.
fn effective_segment_start_at(
    raw_started: DateTime<Utc>,
    now: DateTime<Utc>,
    win_start: DateTime<Utc>,
    win_end: DateTime<Utc>,
    lunch_allowed: u64,
) -> DateTime<Utc> {
    if now >= win_end {
        // 윈도우 밖 (지나감): raw_started 가 윈도우 끝 이전이면 win_end 로 보정.
        if raw_started < win_end {
            return win_end;
        }
        return raw_started;
    }
    if now >= win_start {
        // 윈도우 안: 한도 도달 시각 = max(raw_started, win_start) + allowed
        let exhaust_at = std::cmp::max(raw_started, win_start)
            + chrono::Duration::seconds(lunch_allowed as i64);
        return std::cmp::max(raw_started, exhaust_at);
    }
    // 윈도우 시작 전: 점심 영향 없음.
    raw_started
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn parse_hhmm_basic() {
        assert_eq!(parse_hhmm_to_secs("12:30"), Some(12 * 3600 + 30 * 60));
        assert_eq!(parse_hhmm_to_secs("00:00"), Some(0));
        assert_eq!(parse_hhmm_to_secs("23:59"), Some(23 * 3600 + 59 * 60));
    }

    #[test]
    fn parse_hhmm_invalid() {
        assert_eq!(parse_hhmm_to_secs("24:00"), None);
        assert_eq!(parse_hhmm_to_secs("12:60"), None);
        assert_eq!(parse_hhmm_to_secs("abc"), None);
        assert_eq!(parse_hhmm_to_secs(""), None);
    }

    #[test]
    fn company_seconds_in_day_kst() {
        // 2026-05-14 03:30 UTC = 12:30 KST(+540)
        let t = utc(2026, 5, 14, 3, 30);
        assert_eq!(company_seconds_in_day(t, 540), 12 * 3600 + 30 * 60);
    }

    #[test]
    fn company_seconds_in_day_negative_offset() {
        // 2026-05-14 17:30 UTC = 12:30 PST(-300)
        let t = utc(2026, 5, 14, 17, 30);
        assert_eq!(company_seconds_in_day(t, -300), 12 * 3600 + 30 * 60);
    }

    /// 윈도우 12:30~14:30 (KST), 허용 60분.
    fn kst_window() -> (DateTime<Utc>, DateTime<Utc>) {
        // 2026-05-14 12:30 KST = 03:30 UTC, 14:30 KST = 05:30 UTC
        (utc(2026, 5, 14, 3, 30), utc(2026, 5, 14, 5, 30))
    }

    #[test]
    fn snap_keeps_raw_when_before_window() {
        let (s, e) = kst_window();
        // now=10:00 KST=01:00 UTC, raw=09:00 KST=00:00 UTC
        let now = utc(2026, 5, 14, 1, 0);
        let raw = utc(2026, 5, 14, 0, 0);
        assert_eq!(effective_segment_start_at(raw, now, s, e, 3600), raw);
    }

    #[test]
    fn snap_in_window_quota_exhausted_started_in_window() {
        let (s, e) = kst_window();
        // raw=13:00 KST=04:00 UTC, now=14:00 KST=05:00 UTC, allowed=60min
        // exhaust = max(raw, win_start) + 60min = 13:00 + 60min = 14:00
        let raw = utc(2026, 5, 14, 4, 0);
        let now = utc(2026, 5, 14, 5, 0);
        let result = effective_segment_start_at(raw, now, s, e, 3600);
        assert_eq!(result, utc(2026, 5, 14, 5, 0)); // 14:00 KST
    }

    #[test]
    fn snap_in_window_quota_exhausted_started_before_window() {
        let (s, e) = kst_window();
        // raw=11:00 KST=02:00 UTC, now=13:30 KST=04:30 UTC, allowed=60min
        // (이 경우 IdleOpen 에서 윈도우 진입 시 close 됐어야 하지만 안전망)
        // exhaust = max(raw, win_start) + 60min = win_start + 60min = 13:30 KST
        let raw = utc(2026, 5, 14, 2, 0);
        let now = utc(2026, 5, 14, 4, 30);
        let result = effective_segment_start_at(raw, now, s, e, 3600);
        assert_eq!(result, utc(2026, 5, 14, 4, 30)); // 13:30 KST
    }

    #[test]
    fn snap_after_window_raw_before_window() {
        let (s, e) = kst_window();
        // raw=11:00 KST, now=15:00 KST → started=win_end (14:30 KST=05:30 UTC)
        let raw = utc(2026, 5, 14, 2, 0);
        let now = utc(2026, 5, 14, 6, 0);
        assert_eq!(effective_segment_start_at(raw, now, s, e, 3600), e);
    }

    #[test]
    fn snap_after_window_raw_in_window() {
        let (s, e) = kst_window();
        // raw=13:00 KST, now=15:00 KST → started=win_end
        let raw = utc(2026, 5, 14, 4, 0);
        let now = utc(2026, 5, 14, 6, 0);
        assert_eq!(effective_segment_start_at(raw, now, s, e, 3600), e);
    }

    #[test]
    fn snap_after_window_raw_after_window() {
        let (s, e) = kst_window();
        // raw=14:45 KST, now=15:00 KST → started=raw
        let raw = utc(2026, 5, 14, 5, 45);
        let now = utc(2026, 5, 14, 6, 0);
        assert_eq!(effective_segment_start_at(raw, now, s, e, 3600), raw);
    }
}
