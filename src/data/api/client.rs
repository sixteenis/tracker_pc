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
//! - JSON 파싱 실패 → 응답 본문 일부 포함한 컨텍스트 메시지
//!
//! ── 로깅 ──────────────────────────────────────────────────────────────
//! 모든 HTTP 호출은 `api` 타겟의 tracing 이벤트를 남긴다.
//!   - `info`  : 요청 라인(→ METHOD URL [label]) + 응답 요약(← status 경과ms 크기B)
//!   - `debug` : 응답 본문 미리보기 (PII/토큰 포함 가능 — 운영에서는 info 까지만)
//!   - `warn`  : 4xx/5xx 또는 네트워크 실패
//! 비밀번호 SHA-1 등 민감 query 는 로그에 노출되지 않도록 헬퍼가 base URL 만
//! 출력한다. 호출자가 추가 컨텍스트를 redact 후 trace 레벨로 남길 수 있다.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures::future::{BoxFuture, FutureExt};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use tracing::{info, warn};

use super::endpoints;
use super::ApiClient;
use crate::constants;
use crate::data::dto::login_dto::{LoginRequestDto, LoginResponseDto};
use crate::data::dto::main_info_dto::MainInfoResponseDto;
use crate::data::dto::pay_use_dto::CheckPayUseResponseDto;
use crate::data::dto::*;

pub struct HttpApiClient {
    /// V1 (Resin/JSP). `/android/check_mbr.jsp`, `/android/check_pay_use.jsp`,
    /// `/android/u/get_main2.jsp` 3개 엔드포인트만 여기로 간다.
    base: String,
    /// V2 (Node.js). PC Agent 신규 엔드포인트(정책/이벤트/하트비트/소명/출근/업데이트).
    base_v2: String,
    http: Client,
}

impl HttpApiClient {
    /// reqwest Client 한 번 생성 (V1/V2 커넥션 풀 공유). 타임아웃은 설정값 그대로.
    pub fn new(base_url: String, base_url_v2: String, timeout_seconds: u64) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .user_agent(format!(
                "{}/{}",
                constants::USER_AGENT_PREFIX,
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("reqwest 클라이언트 생성 실패");
        Self {
            base: base_url,
            base_v2: base_url_v2,
            http,
        }
    }

    /// V1 (Resin) 엔드포인트 URL.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    /// V2 (Node.js) 엔드포인트 URL.
    fn url_v2(&self, path: &str) -> String {
        format!("{}{}", self.base_v2.trim_end_matches('/'), path)
    }
}

/// 응답 본문이 길 때 잘라서 로그용 문자열로 변환. UTF-8 경계를 보존한다.
fn body_preview(body: &str) -> String {
    const MAX: usize = 800;
    if body.len() <= MAX {
        return body.to_string();
    }
    let mut end = MAX;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…(+{}B)", &body[..end], body.len() - end)
}

/// 전송 직전의 RequestBuilder 를 try_clone + build 해서 최종 URL(쿼리 포함) 과
/// 요청 본문(JSON 등) 을 미리 꺼낸다. body 가 스트림이거나 build 자체가 실패하면
/// (None, None) 반환 — 그 경우 호출자가 전달한 fallback url 만 로그에 쓰인다.
fn inspect_request(builder: &RequestBuilder) -> (Option<String>, Option<String>) {
    let Some(cloned) = builder.try_clone() else {
        return (None, None);
    };
    let Ok(req) = cloned.build() else {
        return (None, None);
    };
    let url_str = Some(req.url().to_string());
    let body_str = req.body().and_then(|b| b.as_bytes()).map(|bytes| {
        String::from_utf8_lossy(bytes).into_owned()
    });
    (url_str, body_str)
}

/// 요청 → 본문 텍스트 회수 → 상태 코드별 로그 → JSON 파싱.
/// 200~299 면 `T` 로 디시리얼라이즈, 그 외는 `Err`.
async fn send_json<T: DeserializeOwned>(
    builder: RequestBuilder,
    method: &str,
    url: &str,
    label: &'static str,
) -> Result<T> {
    let (full_url_opt, req_body_opt) = inspect_request(&builder);
    let full_url = full_url_opt.as_deref().unwrap_or(url);
    match req_body_opt.as_deref() {
        Some(b) if !b.is_empty() => info!(
            target: "api",
            "→ {} {} [{}] body: {}",
            method,
            full_url,
            label,
            body_preview(b),
        ),
        _ => info!(target: "api", "→ {} {} [{}]", method, full_url, label),
    }

    let started = Instant::now();
    let resp = builder
        .send()
        .await
        .with_context(|| format!("{label} 송신 실패 ({url})"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .with_context(|| format!("{label} 응답 본문 읽기 실패 ({url})"))?;
    let elapsed_ms = started.elapsed().as_millis();

    if status.is_success() {
        info!(
            target: "api",
            "← {} {} [{}] ({}ms, {}B) body: {}",
            status.as_u16(),
            full_url,
            label,
            elapsed_ms,
            body.len(),
            body_preview(&body),
        );
        serde_json::from_str::<T>(&body)
            .with_context(|| format!("{label} JSON 파싱 실패: {}", body_preview(&body)))
    } else {
        log_failure(status, full_url, label, elapsed_ms, &body);
        Err(anyhow!("HTTP {} {}: {}", status, label, body))
    }
}

/// 응답 본문이 없거나 무시해도 되는 호출용. 204 No Content 대응.
async fn send_no_body(
    builder: RequestBuilder,
    method: &str,
    url: &str,
    label: &'static str,
) -> Result<()> {
    let (full_url_opt, req_body_opt) = inspect_request(&builder);
    let full_url = full_url_opt.as_deref().unwrap_or(url);
    match req_body_opt.as_deref() {
        Some(b) if !b.is_empty() => info!(
            target: "api",
            "→ {} {} [{}] body: {}",
            method,
            full_url,
            label,
            body_preview(b),
        ),
        _ => info!(target: "api", "→ {} {} [{}]", method, full_url, label),
    }

    let started = Instant::now();
    let resp = builder
        .send()
        .await
        .with_context(|| format!("{label} 송신 실패 ({url})"))?;
    let status = resp.status();
    let elapsed_ms = started.elapsed().as_millis();

    if status.is_success() {
        info!(
            target: "api",
            "← {} {} [{}] ({}ms)",
            status.as_u16(),
            full_url,
            label,
            elapsed_ms,
        );
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        log_failure(status, full_url, label, elapsed_ms, &body);
        Err(anyhow!("HTTP {} {}: {}", status, label, body))
    }
}

fn log_failure(status: StatusCode, url: &str, label: &str, elapsed_ms: u128, body: &str) {
    warn!(
        target: "api",
        "← {} {} [{}] FAILED ({}ms) body: {}",
        status.as_u16(),
        url,
        label,
        elapsed_ms,
        body_preview(body),
    );
}

impl ApiClient for HttpApiClient {
    fn login<'a>(&'a self, req: LoginRequestDto) -> BoxFuture<'a, Result<LoginResponseDto>> {
        async move {
            let url = self.url(endpoints::LOGIN);
            let email_b64 = B64.encode(req.email.as_bytes());
            let md_b64 = B64.encode(req.device_model.as_bytes());
            // PASS(SHA-1), MODE 외 모든 query 값은 reqwest 가 자동 percent-encode
            // 한다 (BASE64 의 `+`, `/`, `=` 도 안전하게 변환). PASS 는 민감 정보이므로
            // 헬퍼가 base URL 만 로그에 남기도록 한다.
            let query = [
                ("MODE", constants::LOGIN_MODE),
                ("EMAIL", email_b64.as_str()),
                ("PASS", req.password_sha1.as_str()),
                ("OSVS", constants::LOGIN_OSVS),
                ("APPVS", req.app_version.as_str()),
                ("MD", md_b64.as_str()),
            ];
            send_json::<LoginResponseDto>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "login",
            )
            .await
        }
        .boxed()
    }

    fn check_pay_use<'a>(
        &'a self,
        cmpsid: i64,
        mbrsid: i64,
    ) -> BoxFuture<'a, Result<CheckPayUseResponseDto>> {
        async move {
            let url = self.url(endpoints::CHECK_PAY_USE);
            let cmp = cmpsid.to_string();
            let mbr = mbrsid.to_string();
            let query = [("CMPSID", cmp.as_str()), ("MBRSID", mbr.as_str())];
            send_json::<CheckPayUseResponseDto>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "check_pay_use",
            )
            .await
        }
        .boxed()
    }

    fn get_main_info<'a>(
        &'a self,
        empsid: i64,
        cmpsid: i64,
        ttmsid: i64,
        temsid: i64,
    ) -> BoxFuture<'a, Result<MainInfoResponseDto>> {
        async move {
            let url = self.url(endpoints::GET_MAIN);
            let emp = empsid.to_string();
            let cmp = cmpsid.to_string();
            let ttm = ttmsid.to_string();
            let tem = temsid.to_string();
            let query = [
                ("EMPSID", emp.as_str()),
                ("CMPSID", cmp.as_str()),
                ("TTMSID", ttm.as_str()),
                ("TEMSID", tem.as_str()),
            ];
            send_json::<MainInfoResponseDto>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_main_info",
            )
            .await
        }
        .boxed()
    }

    fn get_policy<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move {
            let url = self.url_v2(endpoints::POLICY);
            let emp = emp_sid.to_string();
            let query = [("emp_sid", emp.as_str())];
            send_json::<PolicySnapshot>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_policy",
            )
            .await
        }
        .boxed()
    }

    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>> {
        async move {
            let url = self.url_v2(endpoints::UPDATE_CHECK);
            send_json::<UpdateInfo>(
                self.http.get(&url).query(&req),
                "GET",
                &url,
                "update_check",
            )
            .await
        }
        .boxed()
    }

    fn send_events<'a>(&'a self, batch: EventsBatch) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let url = self.url_v2(endpoints::EVENTS);
            send_json::<EventsBatchResponse>(
                self.http.post(&url).json(&batch),
                "POST",
                &url,
                "send_events",
            )
            .await
        }
        .boxed()
    }

    fn list_explanations<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        async move {
            let url = self.url_v2(endpoints::EXPLANATIONS);
            let emp = emp_sid.to_string();
            let query = [("emp_sid", emp.as_str())];
            send_json::<Vec<RemoteExplanation>>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "list_explanations",
            )
            .await
        }
        .boxed()
    }

    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>> {
        async move {
            let url = self.url_v2(endpoints::EXPLANATIONS);
            send_no_body(
                self.http.post(&url).json(&req),
                "POST",
                &url,
                "submit_explanation",
            )
            .await
        }
        .boxed()
    }

    fn get_attendance<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        async move {
            let url = self.url_v2(endpoints::ATTENDANCE);
            let emp = emp_sid.to_string();
            let query = [("emp_sid", emp.as_str())];
            send_json::<AttendanceSnapshot>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_attendance",
            )
            .await
        }
        .boxed()
    }

    fn get_user_info<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<UserInfoSnapshot>> {
        async move {
            let url = self.url_v2(endpoints::USER_INFO);
            let emp = emp_sid.to_string();
            let query = [("emp_sid", emp.as_str())];
            send_json::<UserInfoSnapshot>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_user_info",
            )
            .await
        }
        .boxed()
    }

    fn get_work_status<'a>(
        &'a self,
        empsid: i64,
    ) -> BoxFuture<'a, Result<crate::data::dto::work_status_dto::WorkStatusResponseDto>> {
        async move {
            let url = self.url(endpoints::GET_WORKSTATUS);
            let emp = empsid.to_string();
            let query = [("EMPSID", emp.as_str())];
            send_json::<crate::data::dto::work_status_dto::WorkStatusResponseDto>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_work_status",
            )
            .await
        }
        .boxed()
    }

    fn patch_policy<'a>(
        &'a self,
        req: PolicyPatchRequest,
    ) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move {
            let url = self.url_v2(endpoints::POLICY);
            send_json::<PolicySnapshot>(
                self.http.patch(&url).json(&req),
                "PATCH",
                &url,
                "patch_policy",
            )
            .await
        }
        .boxed()
    }

    fn create_explanation_type<'a>(
        &'a self,
        req: CreateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>> {
        async move {
            let url = self.url_v2(endpoints::CMS_EXPLANATION_TYPES);
            send_json::<ExplanationType>(
                self.http.post(&url).json(&req),
                "POST",
                &url,
                "create_explanation_type",
            )
            .await
        }
        .boxed()
    }

    fn update_explanation_type<'a>(
        &'a self,
        sid: i64,
        req: PatchExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>> {
        async move {
            let url = format!("{}/{}", self.url_v2(endpoints::CMS_EXPLANATION_TYPES), sid);
            send_json::<ExplanationType>(
                self.http.patch(&url).json(&req),
                "PATCH",
                &url,
                "update_explanation_type",
            )
            .await
        }
        .boxed()
    }

    fn deactivate_explanation_type<'a>(
        &'a self,
        sid: i64,
        req: DeactivateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            let url = format!(
                "{}/{}/deactivate",
                self.url_v2(endpoints::CMS_EXPLANATION_TYPES),
                sid
            );
            // 204 No Content 응답 가정 — body 무시.
            let resp = self
                .http
                .patch(&url)
                .json(&req)
                .send()
                .await
                .map_err(|e| anyhow!("HTTP 실패: {e}"))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow!("HTTP {status}: {}", body_preview(&body)));
            }
            tracing::info!(target: "api", url = %url, %status, "deactivate_explanation_type 응답");
            Ok(())
        }
        .boxed()
    }

    fn get_explanation_usage<'a>(
        &'a self,
        requester_emp_sid: i64,
        days: u32,
    ) -> BoxFuture<'a, Result<Vec<ExplanationUsageEntry>>> {
        async move {
            let url = self.url_v2(endpoints::CMS_EXPLANATION_TYPES_USAGE);
            let emp = requester_emp_sid.to_string();
            let d = days.to_string();
            let query = [("requester_emp_sid", emp.as_str()), ("days", d.as_str())];
            send_json::<Vec<ExplanationUsageEntry>>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "get_explanation_usage",
            )
            .await
        }
        .boxed()
    }

    fn list_explanation_types<'a>(
        &'a self,
        emp_sid: i64,
    ) -> BoxFuture<'a, Result<ExplanationTypesResponse>> {
        async move {
            let url = self.url_v2(endpoints::EXPLANATION_TYPES);
            let emp = emp_sid.to_string();
            let query = [("emp_sid", emp.as_str())];
            send_json::<ExplanationTypesResponse>(
                self.http.get(&url).query(&query),
                "GET",
                &url,
                "list_explanation_types",
            )
            .await
        }
        .boxed()
    }
}
