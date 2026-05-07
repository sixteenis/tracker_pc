//! 점심시간 정책 처리 (기획서 §16).
//!
//! - 점심 가능 시간대 안에서 점심 인정시간 (분) 이하 PC 미사용 → 점심 후보
//! - 점심 인정시간 초과 → 초과분만 소명 대상
//! - 점심 가능 시간대 밖 → 일반 자리비움 소명

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Utc};

use crate::api::types::PolicySnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LunchClassification {
    /// 점심 가능 시간대 안 + 인정시간 이하 — 휴게로 처리, 소명 불필요.
    LunchCandidate,
    /// 점심 가능 시간대 안 + 인정시간 초과 — 초과분 소명 대상.
    LunchExceeded { exceeded_seconds: i64 },
    /// 점심 가능 시간대 밖 — 일반 자리비움 소명.
    Outside,
}

pub fn classify(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    policy: &PolicySnapshot,
) -> LunchClassification {
    let lunch_start = match parse_hhmm(&policy.lunch_start_time) {
        Some(t) => t,
        None => return LunchClassification::Outside,
    };
    let lunch_end = match parse_hhmm(&policy.lunch_end_time) {
        Some(t) => t,
        None => return LunchClassification::Outside,
    };

    // 같은 로컬 날짜로 점심 윈도우를 만든다 (시작 시각 기준).
    let local_start = start.with_timezone(&Local);
    let date = Local
        .with_ymd_and_hms(local_start.year(), local_start.month(), local_start.day(), 0, 0, 0)
        .single();
    let date = match date {
        Some(d) => d,
        None => return LunchClassification::Outside,
    };
    let window_start = date + Duration::seconds(time_seconds(lunch_start));
    let window_end = date + Duration::seconds(time_seconds(lunch_end));

    let window_start = window_start.with_timezone(&Utc);
    let window_end = window_end.with_timezone(&Utc);

    // 자리비움이 점심 윈도우와 전혀 겹치지 않으면 Outside.
    if end <= window_start || start >= window_end {
        return LunchClassification::Outside;
    }

    let overlap_start = start.max(window_start);
    let overlap_end = end.min(window_end);
    let overlap_seconds = (overlap_end - overlap_start).num_seconds().max(0);
    let allowed = (policy.lunch_allowed_minutes as i64) * 60;

    if overlap_seconds <= allowed {
        LunchClassification::LunchCandidate
    } else {
        LunchClassification::LunchExceeded {
            exceeded_seconds: overlap_seconds - allowed,
        }
    }
}

fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

fn time_seconds(t: NaiveTime) -> i64 {
    use chrono::Timelike;
    (t.hour() as i64) * 3600 + (t.minute() as i64) * 60 + (t.second() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::PolicySnapshot;
    use chrono::TimeZone;

    fn policy() -> PolicySnapshot {
        PolicySnapshot {
            policy_version: 1,
            company_idle_threshold_seconds: Some(600),
            team_idle_threshold_seconds: None,
            employee_idle_threshold_seconds: None,
            effective_idle_threshold_seconds: 600,
            policy_scope: "COMPANY".into(),
            lunch_start_time: "11:30".into(),
            lunch_end_time: "14:00".into(),
            lunch_allowed_minutes: 60,
            explanation_deadline_hours: 48,
            heartbeat_interval_seconds: 180,
            event_batch_interval_seconds: 60,
            can_track_time: true,
        }
    }

    fn local_at(h: u32, m: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(2026, 5, 7, h, m, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn within_lunch_under_allowed_is_candidate() {
        let r = classify(local_at(12, 0), local_at(12, 45), &policy());
        assert_eq!(r, LunchClassification::LunchCandidate);
    }

    #[test]
    fn within_lunch_over_allowed_is_exceeded() {
        let r = classify(local_at(12, 0), local_at(13, 30), &policy());
        match r {
            LunchClassification::LunchExceeded { exceeded_seconds } => {
                assert_eq!(exceeded_seconds, 30 * 60);
            }
            other => panic!("expected exceeded, got {other:?}"),
        }
    }

    #[test]
    fn outside_lunch_window() {
        let r = classify(local_at(15, 0), local_at(15, 30), &policy());
        assert_eq!(r, LunchClassification::Outside);
    }
}
