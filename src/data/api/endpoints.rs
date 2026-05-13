//! ============================================================================
//! api::endpoints — 모든 PC Agent 전용 엔드포인트 경로 상수.
//! ============================================================================
//!
//! 서버가 V1/V2 로 분리돼 있다:
//!   - V1 (`config.api.base_url`, env `PINPLE_API_BASE_URL`)
//!     레거시 Resin/JSP 서버. 회원 인증, 요금제 확인, 메인 정보 3개 엔드포인트 전용.
//!   - V2 (`config.api.base_url_v2`, env `PINPLE_API_BASE_URL_V2`)
//!     신규 Node.js 서버. PC Agent 정책/이벤트/하트비트/소명/출근/업데이트 처리.
//!
//! 어느 base 로 가는지는 `api::client::HttpApiClient` 의 메서드에서 `self.url(...)`
//! 또는 `self.url_v2(...)` 로 분기된다.
//!
//! ── 인증 ───────────────────────────────────────────────────────────────
//! `LOGIN` 은 EMAIL(BASE64) + PASS(SHA-1) + OSVS + APPVS + MD(BASE64) 를
//! query string 으로 받는 GET 요청 (`api::client::HttpApiClient::login`).
//! 토큰 기반 갱신은 사용하지 않으며, 자동로그인은 keyring 에 저장된 EMAIL/SHA1
//! 자격 증명으로 동일 LOGIN 엔드포인트를 다시 호출한다.
//!
//! TODO(서버 연동): 다른 엔드포인트 (POLICY/HEARTBEAT/EVENTS/...) 의 인증/요청
//! 형식이 신규 서버 명세와 다르면 여기 + `api::client` 를 함께 수정.

// ── V1 (Resin/JSP) — base_url 사용 ──────────────────────────────────────

/// GET `/android/check_mbr.jsp` — 회원 인증 (로그인).
pub const LOGIN: &str = "/android/check_mbr.jsp";
/// GET `/android/check_pay_use.jsp?CMPSID=&MBRSID=` — 회사 요금제 / 권한 확인.
/// 응답의 `pinpluse` 가 false 면 PC Agent 진입 차단.
pub const CHECK_PAY_USE: &str = "/android/check_pay_use.jsp";
/// GET `/android/u/get_main2.jsp?EMPSID=&CMPSID=&TTMSID=&TEMSID=` — 로그인 후 메인 정보.
pub const GET_MAIN: &str = "/android/u/get_main2.jsp";
/// GET `/android/u/get_workstatus.jsp?EMPSID=` — 출퇴근 상태 판별 (2026-05-12 신규).
/// 응답 `result>0` = 근무중, `result==0` + `main_info` starttm/endtm 조합으로 미출근/퇴근 구분.
pub const GET_WORKSTATUS: &str = "/android/u/get_workstatus.jsp";

// ── V2 (Node.js) — base_url_v2 사용 ─────────────────────────────────────

pub const POLICY: &str = "/api/pc-agent/policy";
pub const UPDATE_CHECK: &str = "/api/pc-agent/update-check";
pub const EVENTS: &str = "/api/pc-agent/events";
pub const EXPLANATIONS: &str = "/api/pc-agent/worktime-explanations";
pub const ATTENDANCE: &str = "/api/pc-agent/attendance-status"; // deprecated → USER_INFO
pub const USER_INFO: &str = "/api/pc-agent/user-info";
/// 회사 커스텀 소명사유 동적 목록 (Phase 1.b, 2026-05-12).
pub const EXPLANATION_TYPES: &str = "/api/pc-agent/explanation-types";
/// 회사 관리자(`Author>=5`) 용 CMS CRUD (Phase 2, 2026-05-12).
pub const CMS_EXPLANATION_TYPES: &str = "/api/cms/pc-agent/explanation-types";
pub const CMS_EXPLANATION_TYPES_USAGE: &str = "/api/cms/pc-agent/explanation-types/usage";
