//! ============================================================================
//! api::client — 실제 HTTP 호출 구현 (`reqwest` 기반).
//! ============================================================================
//!
//! `mock_mode = false` 일 때만 `AppState::new` 가 인스턴스화한다.
//! TLS 는 rustls 사용 (Cargo.toml `rustls-tls` 피처).
//!
//! ── 로그인 흐름 ─────────────────────────────────────────────────────────
//! GET `/android/check_mbr.jsp?MODE=0&EMAIL=<b64>&PASS=<sha1>&OSVS=33&APPVS=<v>&MD=<b64>`
//! - EMAIL: 이메일 평문을 BASE64 standard 로 인코딩
//! - PASS : 비밀번호의 SHA-1 해시 (40자 hex)
//! - MD   : 단말기 모델명을 BASE64 standard 로 인코딩
//!
//! 응답 본문은 200 OK 라도 `mbrsid <= 0` 이면 실패 — 호출자(`auth::login`) 가
//! `LoginResponseRaw::is_success` 로 검사한다.
//!
//! ── 에러 처리 ──────────────────────────────────────────────────────────
//! - 네트워크 에러 → `anyhow!("HTTP {status}: {body}")`
//! - JSON 파싱 실패 → reqwest 에러 그대로

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures::future::{BoxFuture, FutureExt};
use reqwest::Client;

use super::endpoints;
use super::ApiClient;
use crate::constants;
use crate::data::dto::login_dto::{LoginRequestDto, LoginResponseDto};
use crate::data::dto::*;

pub struct HttpApiClient {
    base: String,
    http: Client,
}

impl HttpApiClient {
    /// reqwest Client 한 번 생성 (커넥션 풀 공유). 타임아웃은 설정값 그대로.
    pub fn new(base_url: String, timeout_seconds: u64) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .user_agent(format!(
                "{}/{}",
                constants::USER_AGENT_PREFIX,
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("reqwest 클라이언트 생성 실패");
        Self { base: base_url, http }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }
}

async fn check_ok(resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow!("HTTP {status}: {body}"))
    }
}

impl ApiClient for HttpApiClient {
    fn login<'a>(&'a self, req: LoginRequestDto) -> BoxFuture<'a, Result<LoginResponseDto>> {
        async move {
            let email_b64 = B64.encode(req.email.as_bytes());
            let md_b64 = B64.encode(req.device_model.as_bytes());
            // PASS, MODE 외 모든 query 값은 reqwest 가 자동 percent-encode 한다
            // (BASE64 의 `+`, `/`, `=` 도 안전하게 변환).
            let query = [
                ("MODE", constants::LOGIN_MODE),
                ("EMAIL", email_b64.as_str()),
                ("PASS", req.password_sha1.as_str()),
                ("OSVS", constants::LOGIN_OSVS),
                ("APPVS", req.app_version.as_str()),
                ("MD", md_b64.as_str()),
            ];
            let resp = self
                .http
                .get(self.url(endpoints::LOGIN))
                .query(&query)
                .send()
                .await
                .context("로그인 요청 실패")?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<LoginResponseDto>().await?)
        }
        .boxed()
    }

    fn get_policy<'a>(&'a self) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::POLICY))
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<PolicySnapshot>().await?)
        }
        .boxed()
    }

    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::UPDATE_CHECK))
                .query(&req)
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<UpdateInfo>().await?)
        }
        .boxed()
    }

    fn send_heartbeat<'a>(
        &'a self,
        beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::HEARTBEAT))
                .json(&beat)
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<HeartbeatResponse>().await?)
        }
        .boxed()
    }

    fn send_events<'a>(&'a self, batch: EventsBatch) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::EVENTS))
                .json(&batch)
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<EventsBatchResponse>().await?)
        }
        .boxed()
    }

    fn list_explanations<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::EXPLANATIONS))
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<Vec<RemoteExplanation>>().await?)
        }
        .boxed()
    }

    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::EXPLANATIONS))
                .json(&req)
                .send()
                .await?;
            check_ok(resp).await?;
            Ok(())
        }
        .boxed()
    }

    fn get_attendance<'a>(&'a self) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::ATTENDANCE))
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<AttendanceSnapshot>().await?)
        }
        .boxed()
    }
}
