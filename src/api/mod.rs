//! 서버 통신 추상화. `mock_mode = true` 면 `MockClient` 가 주입되어
//! 네트워크 호출 없이 기본 응답을 반환한다.
//!
//! 모든 메서드는 `BoxFuture` 를 반환하므로 `dyn ApiClient` 로 안전하게 보관 가능.

pub mod client;
pub mod endpoints;
pub mod mock;
pub mod types;

use anyhow::Result;
use futures::future::BoxFuture;

use types::*;

pub trait ApiClient: Send + Sync {
    fn login<'a>(&'a self, req: LoginRequest) -> BoxFuture<'a, Result<LoginResponse>>;

    fn refresh<'a>(&'a self, req: RefreshRequest) -> BoxFuture<'a, Result<LoginResponse>>;

    fn get_policy<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<PolicySnapshot>>;

    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>>;

    fn send_heartbeat<'a>(
        &'a self,
        access_token: &'a str,
        beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>>;

    fn send_events<'a>(
        &'a self,
        access_token: &'a str,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>>;

    fn list_explanations<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>>;

    fn submit_explanation<'a>(
        &'a self,
        access_token: &'a str,
        req: ExplanationSubmit,
    ) -> BoxFuture<'a, Result<()>>;

    fn get_attendance<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<AttendanceSnapshot>>;
}
