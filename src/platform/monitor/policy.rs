//! ============================================================================
//! monitor::policy — 자리비움 기준 시간 우선순위 해석 (기획서 §13).
//! ============================================================================
//!
//!   1. employee → 2. team → 3. company → 4. default
//!
//! 서버는 보통 `effective_idle_threshold_seconds` + `policy_scope` 두 값을 직접
//! 계산해서 내려주지만, 만일 그 두 값이 없거나 신뢰할 수 없다면 아래 함수로
//! 클라이언트가 재계산한다 (방어 코드 / 테스트).
//!
//! TODO(2차): 정책 변경 감지 시 진행 중인 자리비움 segment 의 처리 정책 합의.
//! 현재는 새로 생성되는 segment 만 새 기준 적용 — 진행 중 segment 는 시작 시점
//! 기준 그대로. 관리자/근로자 입장에서 어떤 동작이 자연스러운지 결정 필요.

use crate::data::dto::PolicySnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Employee,
    Team,
    Company,
    Default,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Employee => "EMPLOYEE",
            Self::Team => "TEAM",
            Self::Company => "COMPANY",
            Self::Default => "DEFAULT",
        }
    }
}

/// 정책 스냅샷에서 적용할 임계값 (초) + scope 라벨 반환.
/// 서버가 내려준 `effective_idle_threshold_seconds` 가 정확하지 않을 때만 사용.
pub fn resolve(policy: &PolicySnapshot, default_seconds: u64) -> (u64, Scope) {
    if let Some(v) = policy.employee_idle_threshold_seconds {
        return (v, Scope::Employee);
    }
    if let Some(v) = policy.team_idle_threshold_seconds {
        return (v, Scope::Team);
    }
    if let Some(v) = policy.company_idle_threshold_seconds {
        return (v, Scope::Company);
    }
    (default_seconds, Scope::Default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dto::PolicySnapshot;

    fn p(c: Option<u64>, t: Option<u64>, e: Option<u64>) -> PolicySnapshot {
        PolicySnapshot {
            policy_version: 0,
            company_idle_threshold_seconds: c,
            team_idle_threshold_seconds: t,
            employee_idle_threshold_seconds: e,
            effective_idle_threshold_seconds: 0,
            policy_scope: String::new(),
            lunch_start_time: "11:30".to_string(),
            lunch_end_time: "14:00".to_string(),
            lunch_allowed_minutes: 60,
            explanation_deadline_hours: 48,
            can_track_time: true,
        }
    }

    #[test]
    fn employee_wins() {
        let (v, s) = resolve(&p(Some(600), Some(900), Some(450)), 600);
        assert_eq!(v, 450);
        assert_eq!(s, Scope::Employee);
    }

    #[test]
    fn team_when_no_employee() {
        let (v, s) = resolve(&p(Some(600), Some(900), None), 600);
        assert_eq!(v, 900);
        assert_eq!(s, Scope::Team);
    }

    #[test]
    fn company_when_no_team() {
        let (v, s) = resolve(&p(Some(600), None, None), 600);
        assert_eq!(v, 600);
        assert_eq!(s, Scope::Company);
    }

    #[test]
    fn default_when_nothing() {
        let (v, s) = resolve(&p(None, None, None), 600);
        assert_eq!(v, 600);
        assert_eq!(s, Scope::Default);
    }
}
