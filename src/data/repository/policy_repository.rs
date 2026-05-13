//! 정책 repository — 현재 mock. 신규 서버 정책 명세 합의 후 실제 호출로 교체.

use std::time::Duration;

use crate::domain::model::policy::{Policy, PolicyScope};

/// 임시 mock 정책. 테스트 편의를 위해 임계값이 낮음(5초).
pub fn mock_policy() -> Policy {
    Policy {
        version: 1,
        idle_threshold: Duration::from_secs(5),
        scope: PolicyScope::Company,
        lunch_start: "11:30".to_string(),
        lunch_end: "14:00".to_string(),
        lunch_allowed_minutes: 60,
        explanation_deadline_hours: 48,
        can_track_time: true,
    }
}
