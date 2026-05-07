//! ============================================================================
//! api::mock — 개발용 Mock API. `mock_mode = true` 일 때 주입.
//! ============================================================================
//!
//! 모든 응답은 "성공" 시나리오를 가정한다 (기획서 마지막 단락 참고).
//! - 로그인 성공
//! - 요금제 활성 (`can_track_time = true`)
//! - 정책 조회 성공 (effective_idle_threshold_seconds = 5초, scope = COMPANY  ← 테스트용)
//! - 이벤트 전송 성공 (모든 event_id accept)
//! - 소명 제출 성공
//! - 출근 상태 = WORKING
//! - 업데이트 = 최신 (force_update = false)
//!
//! ── 테스트 시나리오 시뮬레이션 방법 ─────────────────────────────────────
//! - `force_update = true` 시뮬: `update_check` 함수에서 응답 임시 수정
//! - 요금제 미포함 시뮬: `fake_policy().can_track_time = false`
//! - 다른 PC 강제 로그아웃 시뮬: `send_heartbeat` 응답에 `force_logout = true`
//!
//! TODO(테스트): 위 분기들을 한 번에 토글할 수 있는 환경변수 추가
//! (예: `PINPLE_MOCK_FORCE_UPDATE=1`).
//! TODO(테스트): `fixtures/` 디렉토리에 시나리오별 JSON 응답 두고 환경변수로 선택.

use std::sync::Mutex;

use anyhow::Result;
use chrono::Utc;
use futures::future::{BoxFuture, FutureExt};
use uuid::Uuid;

use super::types::*;
use super::ApiClient;

pub struct MockClient {
    submitted: Mutex<Vec<ExplanationSubmit>>,
}

impl MockClient {
    pub fn new() -> Self {
        Self { submitted: Mutex::new(Vec::new()) }
    }

    /// access_token / refresh_token / 만료 초.
    fn fake_session() -> (String, String, i64) {
        (
            format!("mock-access-{}", Uuid::new_v4()),
            format!("mock-refresh-{}", Uuid::new_v4()),
            3600,
        )
    }

    /// 회사 정책 더미. 테스트 편의를 위해 임계값이 매우 낮음(5초).
    /// 운영 시연 전 600(10분) 등 현실 값으로 복원할 것.
    fn fake_policy() -> PolicySnapshot {
        // ※ 테스트용: idle 임계값을 5초로 내려서 즉시 자리비움 검증이 가능하도록.
        //   운영 또는 시연 시에는 600(10분) 이상으로 복원할 것.
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
}

impl ApiClient for MockClient {
    fn login<'a>(&'a self, req: LoginRequest) -> BoxFuture<'a, Result<LoginResponse>> {
        async move {
            let (access, refresh, ttl) = Self::fake_session();
            Ok(LoginResponse {
                access_token: access,
                refresh_token: refresh,
                access_token_expires_in: ttl,
                company_id: "MOCK_CO_001".to_string(),
                employee_id: req.login_id,
                employee_name: Some("홍길동 (Mock)".to_string()),
                team_id: Some("TEAM_DEV".to_string()),
                team_name: Some("개발팀".to_string()),
                subscription: SubscriptionInfo {
                    plan_code: "PRO".to_string(),
                    payment_status: "ACTIVE".to_string(),
                    pc_tracking_enabled: true,
                    can_track_time: true,
                },
                policy: Self::fake_policy(),
                displaced_device: None,
            })
        }
        .boxed()
    }

    fn refresh<'a>(&'a self, _req: RefreshRequest) -> BoxFuture<'a, Result<LoginResponse>> {
        async move {
            let (access, refresh, ttl) = Self::fake_session();
            Ok(LoginResponse {
                access_token: access,
                refresh_token: refresh,
                access_token_expires_in: ttl,
                company_id: "MOCK_CO_001".to_string(),
                employee_id: "mock-user".to_string(),
                employee_name: Some("홍길동 (Mock)".to_string()),
                team_id: Some("TEAM_DEV".to_string()),
                team_name: Some("개발팀".to_string()),
                subscription: SubscriptionInfo {
                    plan_code: "PRO".to_string(),
                    payment_status: "ACTIVE".to_string(),
                    pc_tracking_enabled: true,
                    can_track_time: true,
                },
                policy: Self::fake_policy(),
                displaced_device: None,
            })
        }
        .boxed()
    }

    fn get_policy<'a>(&'a self, _access_token: &'a str) -> BoxFuture<'a, Result<PolicySnapshot>> {
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
        _access_token: &'a str,
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

    fn send_events<'a>(
        &'a self,
        _access_token: &'a str,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let ids = batch.events.into_iter().map(|e| e.event_id).collect();
            Ok(EventsBatchResponse { accepted_event_ids: ids })
        }
        .boxed()
    }

    fn list_explanations<'a>(
        &'a self,
        _access_token: &'a str,
    ) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        // 빈 목록 — 로컬 DB의 자리비움 구간만 표시.
        async move { Ok(Vec::new()) }.boxed()
    }

    fn submit_explanation<'a>(
        &'a self,
        _access_token: &'a str,
        req: ExplanationSubmit,
    ) -> BoxFuture<'a, Result<()>> {
        async move {
            self.submitted.lock().unwrap().push(req);
            Ok(())
        }
        .boxed()
    }

    fn get_attendance<'a>(
        &'a self,
        _access_token: &'a str,
    ) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        // 개발 편의를 위해 항상 "출근 중" 으로 반환.
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
