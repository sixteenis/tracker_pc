//! 실제 HTTP 호출 구현. `mock_mode = false` 일 때만 사용.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures::future::{BoxFuture, FutureExt};
use reqwest::{header, Client, StatusCode};

use super::endpoints;
use super::types::*;
use super::ApiClient;

pub struct HttpApiClient {
    base: String,
    http: Client,
}

impl HttpApiClient {
    pub fn new(base_url: String, timeout_seconds: u64) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .user_agent(format!("PinplePCAgent/{}", env!("CARGO_PKG_VERSION")))
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
    fn login<'a>(&'a self, req: LoginRequest) -> BoxFuture<'a, Result<LoginResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::LOGIN))
                .json(&req)
                .send()
                .await
                .context("로그인 요청 실패")?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<LoginResponse>().await?)
        }
        .boxed()
    }

    fn refresh<'a>(&'a self, req: RefreshRequest) -> BoxFuture<'a, Result<LoginResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::REFRESH))
                .json(&req)
                .send()
                .await?;
            // 401 → refresh 만료. 호출 측이 로그인 화면으로 전환.
            if resp.status() == StatusCode::UNAUTHORIZED {
                return Err(anyhow!("REFRESH_EXPIRED"));
            }
            let resp = check_ok(resp).await?;
            Ok(resp.json::<LoginResponse>().await?)
        }
        .boxed()
    }

    fn get_policy<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::POLICY))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
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
        access_token: &'a str,
        beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::HEARTBEAT))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .json(&beat)
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<HeartbeatResponse>().await?)
        }
        .boxed()
    }

    fn send_events<'a>(
        &'a self,
        access_token: &'a str,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::EVENTS))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .json(&batch)
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<EventsBatchResponse>().await?)
        }
        .boxed()
    }

    fn list_explanations<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::EXPLANATIONS))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<Vec<RemoteExplanation>>().await?)
        }
        .boxed()
    }

    fn submit_explanation<'a>(
        &'a self,
        access_token: &'a str,
        req: ExplanationSubmit,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            let resp = self
                .http
                .post(self.url(endpoints::EXPLANATIONS))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .json(&req)
                .send()
                .await?;
            check_ok(resp).await?;
            Ok(())
        }
        .boxed()
    }

    fn get_attendance<'a>(
        &'a self,
        access_token: &'a str,
    ) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        async move {
            let resp = self
                .http
                .get(self.url(endpoints::ATTENDANCE))
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await?;
            let resp = check_ok(resp).await?;
            Ok(resp.json::<AttendanceSnapshot>().await?)
        }
        .boxed()
    }
}
