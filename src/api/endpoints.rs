//! 모든 PC Agent 전용 엔드포인트 경로 (기획서 §22, §25).

pub const LOGIN: &str = "/api/pc-agent/login";
pub const REFRESH: &str = "/api/pc-agent/refresh";
pub const POLICY: &str = "/api/pc-agent/policy";
pub const UPDATE_CHECK: &str = "/api/pc-agent/update-check";
pub const HEARTBEAT: &str = "/api/pc-agent/heartbeat";
pub const EVENTS: &str = "/api/pc-agent/events";
pub const EXPLANATIONS: &str = "/api/pc-agent/worktime-explanations";
pub const ATTENDANCE: &str = "/api/pc-agent/attendance-status";
