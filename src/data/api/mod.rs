//! ============================================================================
//! data::api — 서버 통신 추상화 레이어.
//! ============================================================================
//!
//! `ApiClient` trait 으로 PC Agent 가 사용하는 엔드포인트들을 추상화.
//! `mock_mode = true` 면 `MockClient` 가 주입되어 네트워크 호출 없이 기본 응답을 반환.
//!
//! ── 인증 모델 ──────────────────────────────────────────────────────────
//! `login()` 만 EMAIL(BASE64) + PASS(SHA-1) + OSVS + APPVS + MD(BASE64) 로
//! 호출되며, 다른 엔드포인트는 신규 서버 명세 합의 전이라 인증 파라미터 미정.
//! 합의되면 trait 시그니처에 mbrsid / empsid 등을 추가.

pub mod client;
pub mod endpoints;
pub mod mock;

use anyhow::Result;
use futures::future::BoxFuture;

use crate::data::dto::login_dto::{LoginRequestDto, LoginResponseDto};
use crate::data::dto::*;

pub trait ApiClient: Send + Sync {
    /// GET `/android/check_mbr.jsp` — 이메일 + SHA1 PASS + 디바이스 메타로 로그인.
    /// `LoginResponseDto::is_success()` 가 false 인 경우(`mbrsid <= 0`) 응답은
    /// 정상이지만 인증 실패. 호출자(`auth_repository::login`) 가 분기 처리.
    fn login<'a>(&'a self, req: LoginRequestDto) -> BoxFuture<'a, Result<LoginResponseDto>>;

    /// GET /api/pc-agent/policy — 자리비움 기준/점심 정책 등 재조회.
    /// TODO(서버 연동): 인증 파라미터(mbrsid/empsid 등) 확정.
    fn get_policy<'a>(&'a self) -> BoxFuture<'a, Result<PolicySnapshot>>;

    /// GET /api/pc-agent/update-check — 최신 버전/강제 업데이트 정보.
    /// 인증 헤더 불필요.
    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>>;

    /// POST /api/pc-agent/heartbeat — 3분 주기 PC 상태 보고.
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn send_heartbeat<'a>(
        &'a self,
        beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>>;

    /// POST /api/pc-agent/events — 의미 있는 이벤트 배치 전송 (1분 주기, 최대 50건).
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn send_events<'a>(
        &'a self,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>>;

    /// GET /api/pc-agent/worktime-explanations — 서버 측 자리비움/소명 목록.
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn list_explanations<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>>;

    /// POST /api/pc-agent/worktime-explanations — 사용자가 입력한 소명 제출.
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>>;

    /// GET /api/pc-agent/attendance-status — 오늘 출근 상태 조회.
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn get_attendance<'a>(&'a self) -> BoxFuture<'a, Result<AttendanceSnapshot>>;
}
