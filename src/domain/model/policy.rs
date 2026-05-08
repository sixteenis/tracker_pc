//! 정책 도메인 모델 — 자리비움 기준 / 점심 정책 / 소명 마감.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Policy {
    /// 서버가 보내준 정책 버전 (변경 감지용).
    pub version: i64,
    /// 적용된 자리비움 임계값 (초).
    pub idle_threshold: Duration,
    /// 임계값이 어느 scope 에서 결정됐는지 — DEFAULT / COMPANY / TEAM / EMPLOYEE.
    pub scope: PolicyScope,
    /// 점심 시간 윈도우 시작 ("HH:MM").
    pub lunch_start: String,
    /// 점심 시간 윈도우 끝 ("HH:MM").
    pub lunch_end: String,
    /// 점심 인정 분 수.
    pub lunch_allowed_minutes: u32,
    /// 자리비움 발생 후 소명 가능한 시간 (시간 단위).
    pub explanation_deadline_hours: u32,
    /// 회사/요금제 단에서 PC 시간 추적이 켜져 있는지.
    pub can_track_time: bool,

    /// heartbeat 권장 주기 (초).
    pub heartbeat_interval_seconds: u64,
    /// 이벤트 배치 권장 주기 (초).
    pub event_batch_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyScope {
    Default,
    Company,
    Team,
    Employee,
}

impl PolicyScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Company => "COMPANY",
            Self::Team => "TEAM",
            Self::Employee => "EMPLOYEE",
        }
    }

    pub fn from_wire(s: &str) -> Self {
        match s {
            "EMPLOYEE" => Self::Employee,
            "TEAM" => Self::Team,
            "COMPANY" => Self::Company,
            _ => Self::Default,
        }
    }
}
