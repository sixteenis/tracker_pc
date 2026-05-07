//! ============================================================================
//! api — 서버 통신 추상화 레이어.
//! ============================================================================
//!
//! `ApiClient` trait 으로 9개 엔드포인트(기획서 §22, §25) 추상화.
//! `mock_mode = true` 면 `MockClient` 가 주입되어 네트워크 호출 없이 기본
//! 응답을 반환한다.
//!
//! 모든 메서드는 `BoxFuture` 를 반환하므로 `Arc<dyn ApiClient>` 로 안전하게
//! 보관 가능 (런타임 dispatch).
//!
//! ── 인증 ───────────────────────────────────────────────────────────────
//! `login`/`refresh`/`update_check` 외 메서드는 `access_token` 을 받아
//! `Authorization: Bearer <token>` 헤더로 전달한다.
//!
//! TODO(서버 연동): 9개 엔드포인트 모두 핀플 백엔드와 명세 합의 필요. 현재는
//! 기획서 기준으로 PC Agent 측이 기대하는 형태로만 구현됨. 실 서버 endpoint 와
//! 차이 있으면 `types.rs` 의 DTO + 본 trait 시그니처 동시 수정.
//! TODO(2차): 401 Unauthorized 자동 refresh 후 재시도 미들웨어 (현재는 호출자가
//! 매번 access_token 만료 처리해야 함).
//! TODO(2차): rate-limit / 백오프 (서버가 429 응답 시).

pub mod client;
pub mod endpoints;
pub mod mock;
pub mod types;

use anyhow::Result;
use futures::future::BoxFuture;

use types::*;

pub trait ApiClient: Send + Sync {
    /// POST /api/pc-agent/login — 아이디/비밀번호 + device 정보 로그인.
    /// 응답에 access/refresh 토큰 + 사용자/회사/팀/구독/정책 포함.
    fn login<'a>(&'a self, req: LoginRequest) -> BoxFuture<'a, Result<LoginResponse>>;

    /// POST /api/pc-agent/refresh — refresh_token 으로 access_token 재발급.
    /// 401 응답 시 호출자는 로그인 화면으로 전환해야 한다 (auth::try_auto_login 처리).
    fn refresh<'a>(&'a self, req: RefreshRequest) -> BoxFuture<'a, Result<LoginResponse>>;

    /// GET /api/pc-agent/policy — 자리비움 기준/점심 정책 등 재조회.
    /// `sync::policy_sync` 가 30분마다 호출.
    fn get_policy<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<PolicySnapshot>>;

    /// GET /api/pc-agent/update-check — 최신 버전/강제 업데이트 정보.
    /// 인증 헤더 불필요 (퍼블릭 메타데이터).
    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>>;

    /// POST /api/pc-agent/heartbeat — 3분 주기 PC 상태 보고.
    /// 응답으로 다음 주기 / policy_version / can_track_time / force_logout 받음.
    fn send_heartbeat<'a>(
        &'a self,
        access_token: &'a str,
        beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>>;

    /// POST /api/pc-agent/events — 의미 있는 이벤트 배치 전송 (1분 주기, 최대 50건).
    /// 응답의 `accepted_event_ids` 가 SUCCESS 처리 기준.
    fn send_events<'a>(
        &'a self,
        access_token: &'a str,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>>;

    /// GET /api/pc-agent/worktime-explanations — 서버 측 자리비움/소명 목록.
    /// 1차 MVP 에서는 로컬 `idle_segments` 만 표시하므로 거의 사용 안 함.
    /// TODO(2차): 로컬 segment 와 병합해서 다른 PC 에서 만든 segment 도 표시.
    fn list_explanations<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>>;

    /// POST /api/pc-agent/worktime-explanations — 사용자가 입력한 소명 제출.
    fn submit_explanation<'a>(
        &'a self,
        access_token: &'a str,
        req: ExplanationSubmit,
    ) -> BoxFuture<'a, Result<()>>;

    /// GET /api/pc-agent/attendance-status — 오늘 출근 상태 조회.
    /// 5분 주기 `sync::attendance_sync` 가 호출. PC 앱은 출근/퇴근을 변경하지 않음 (read-only).
    fn get_attendance<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<AttendanceSnapshot>>;
}
