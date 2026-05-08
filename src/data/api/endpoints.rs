//! ============================================================================
//! api::endpoints — 모든 PC Agent 전용 엔드포인트 경로 상수.
//! ============================================================================
//!
//! base_url 은 `config.api.base_url`. 환경변수 `PINPLE_API_BASE_URL` 으로 오버라이드.
//!
//! ── 인증 ───────────────────────────────────────────────────────────────
//! `LOGIN` 은 EMAIL(BASE64) + PASS(SHA-1) + OSVS + APPVS + MD(BASE64) 를
//! query string 으로 받는 GET 요청 (`api::client::HttpApiClient::login`).
//! 토큰 기반 갱신은 사용하지 않으며, 자동로그인은 keyring 에 저장된 EMAIL/SHA1
//! 자격 증명으로 동일 LOGIN 엔드포인트를 다시 호출한다.
//!
//! TODO(서버 연동): 다른 엔드포인트 (POLICY/HEARTBEAT/EVENTS/...) 의 인증/요청
//! 형식이 신규 서버 명세와 다르면 여기 + `api::client` 를 함께 수정.

pub const LOGIN: &str = "/android/check_mbr.jsp";
pub const POLICY: &str = "/api/pc-agent/policy";
pub const UPDATE_CHECK: &str = "/api/pc-agent/update-check";
pub const HEARTBEAT: &str = "/api/pc-agent/heartbeat";
pub const EVENTS: &str = "/api/pc-agent/events";
pub const EXPLANATIONS: &str = "/api/pc-agent/worktime-explanations";
pub const ATTENDANCE: &str = "/api/pc-agent/attendance-status";
