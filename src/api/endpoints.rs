//! ============================================================================
//! api::endpoints — 모든 PC Agent 전용 엔드포인트 경로 상수.
//! ============================================================================
//!
//! 기획서 §22, §25 참고. base_url 은 `config.api.base_url`. 환경변수
//! `PINPLE_API_BASE_URL` 으로 오버라이드 가능.
//!
//! TODO(서버 연동): 핀플 백엔드 팀과 경로 prefix(`/api/pc-agent/...`) 합의.
//! 만약 v2 등 versioning 들어가면 여기서만 변경.

pub const LOGIN: &str = "/api/pc-agent/login";
pub const REFRESH: &str = "/api/pc-agent/refresh";
pub const POLICY: &str = "/api/pc-agent/policy";
pub const UPDATE_CHECK: &str = "/api/pc-agent/update-check";
pub const HEARTBEAT: &str = "/api/pc-agent/heartbeat";
pub const EVENTS: &str = "/api/pc-agent/events";
pub const EXPLANATIONS: &str = "/api/pc-agent/worktime-explanations";
pub const ATTENDANCE: &str = "/api/pc-agent/attendance-status";
