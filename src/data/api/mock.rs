//! ============================================================================
//! api::mock — 개발용 Mock API. `mock_mode = true` 일 때 주입.
//! ============================================================================
//!
//! 모든 응답은 "성공" 시나리오를 가정한다.
//! - 로그인 성공 (mbrsid > 0)
//! - 정책 조회 성공 (effective_idle_threshold_seconds = 5초)
//! - 이벤트 전송 성공 (모든 event_id accept)
//! - 소명 제출 성공
//! - 출근 상태 = WORKING
//! - 업데이트 = 최신
//!
//! ── 실패 시나리오 시뮬 ──────────────────────────────────────────────────
//! `MockClient::set_login_failure(true)` (TODO) — 추후 환경변수로 토글 추가.

use std::sync::Mutex;

use anyhow::Result;
use chrono::Utc;
use futures::future::{BoxFuture, FutureExt};

use super::ApiClient;
use crate::data::dto::login_dto::{LoginRequestDto, LoginResponseDto};
use crate::data::dto::*;

pub struct MockClient {
    submitted: Mutex<Vec<ExplanationSubmit>>,
}

impl MockClient {
    pub fn new() -> Self {
        Self { submitted: Mutex::new(Vec::new()) }
    }

    /// 회사 정책 더미. 테스트 편의를 위해 임계값이 매우 낮음(5초).
    /// 운영 시연 전 600(10분) 등 현실 값으로 복원할 것.
    fn fake_policy() -> PolicySnapshot {
        PolicySnapshot {
            policy_version: 1,
            company_idle_threshold_seconds: Some(5),
            team_idle_threshold_seconds: None,
            employee_idle_threshold_seconds: None,
            effective_idle_threshold_seconds: 5,
            policy_scope: "COMPANY".to_string(),
            lunch_start_time: "11:30".to_string(),
            lunch_end_time: "14:00".to_string(),
            lunch_allowed_minutes: 60,
            explanation_deadline_hours: 48,
            heartbeat_interval_seconds: 180,
            event_batch_interval_seconds: 60,
            can_track_time: true,
        }
    }

    /// 성공 응답 더미. 실제 서버의 응답 예시를 거의 그대로 사용 (mbrsid > 0).
    fn fake_member(email: &str) -> LoginResponseDto {
        LoginResponseDto {
            mbrsid: 55725,
            empsid: 48660,
            cmpsid: 11402,
            temsid: 9869,
            ttmsid: 3221,
            email: email.to_string(),
            name: "박일일".to_string(),
            enname: "".to_string(),
            ttmname: "성민".to_string(),
            cmpname: "성민".to_string(),
            temname: "개발".to_string(),
            gender: 0,
            birth: "1900-01-01".to_string(),
            phonenum: "01011112222".to_string(),
            bcemail: "".to_string(),
            empnum: "".to_string(),
            spot: "".to_string(),
            author: 5,
            lunar: 0,
            notrc: 0,
            regdt: "2026-02-02".to_string(),
            joindt: "2020-02-02".to_string(),
            profimg: "".to_string(),
            pushid: "".to_string(),
            curver: "1.5.14".to_string(),
            update: 0,
            updatemsg: "".to_string(),
        }
    }
}

impl ApiClient for MockClient {
    fn login<'a>(&'a self, req: LoginRequestDto) -> BoxFuture<'a, Result<LoginResponseDto>> {
        async move { Ok(Self::fake_member(&req.email)) }.boxed()
    }

    fn get_policy<'a>(&'a self) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move { Ok(Self::fake_policy()) }.boxed()
    }

    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>> {
        async move {
            Ok(UpdateInfo {
                current_version: req.current_version.clone(),
                latest_version: req.current_version,
                minimum_required_version: "0.1.0".to_string(),
                update_required: false,
                force_update: false,
                download_url: String::new(),
                release_note: "Mock 모드 — 업데이트 없음".to_string(),
            })
        }
        .boxed()
    }

    fn send_heartbeat<'a>(
        &'a self,
        _beat: HeartbeatRequest,
    ) -> BoxFuture<'a, Result<HeartbeatResponse>> {
        async move {
            Ok(HeartbeatResponse {
                next_heartbeat_seconds: 180,
                policy_version: 1,
                can_track_time: true,
                force_logout: false,
            })
        }
        .boxed()
    }

    fn send_events<'a>(&'a self, batch: EventsBatch) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let ids = batch.events.into_iter().map(|e| e.event_id).collect();
            Ok(EventsBatchResponse { accepted_event_ids: ids })
        }
        .boxed()
    }

    fn list_explanations<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        async move { Ok(Vec::new()) }.boxed()
    }

    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>> {
        async move {
            self.submitted.lock().unwrap().push(req);
            Ok(())
        }
        .boxed()
    }

    fn get_attendance<'a>(&'a self) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        async move {
            Ok(AttendanceSnapshot {
                attendance_status: AttendanceStatus::Working,
                work_start_at: Some(Utc::now()),
                work_end_at: None,
            })
        }
        .boxed()
    }
}
