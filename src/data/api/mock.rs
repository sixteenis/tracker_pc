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
use crate::data::dto::main_info_dto::MainInfoResponseDto;
use crate::data::dto::pay_use_dto::CheckPayUseResponseDto;
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
            can_track_time: true,
            time_zone_offset_minutes: 540,
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

    fn check_pay_use<'a>(
        &'a self,
        _cmpsid: i64,
        _mbrsid: i64,
    ) -> BoxFuture<'a, Result<CheckPayUseResponseDto>> {
        async move { Ok(CheckPayUseResponseDto { pinpluse: true, payuse: true }) }.boxed()
    }

    fn get_main_info<'a>(
        &'a self,
        _empsid: i64,
        _cmpsid: i64,
        _ttmsid: i64,
        _temsid: i64,
    ) -> BoxFuture<'a, Result<MainInfoResponseDto>> {
        async move {
            Ok(MainInfoResponseDto {
                starttm: "09:00".to_string(),
                endtm: "18:00".to_string(),
                joindt: "2023-11-09".to_string(),
                anual: 7200,
                workmin: 0,
                addmin: 0,
                usemin: 0,
                msgcnt: 0,
                anualddctn1: 1,
                anualddctn2: 1,
                anualddctn3: 1,
                st_anual: 1,
                st_d_anual: 1,
                brk_time: 1,
                schdl: 1,
                cmt_lt: 2,
                cmtnoti: 0,
                wk52h: 0,
            })
        }
        .boxed()
    }

    fn get_policy<'a>(&'a self, _emp_sid: i64) -> BoxFuture<'a, Result<PolicySnapshot>> {
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

    fn send_events<'a>(&'a self, batch: EventsBatch) -> BoxFuture<'a, Result<EventsBatchResponse>> {
        async move {
            let ids = batch.events.into_iter().map(|e| e.event_id).collect();
            Ok(EventsBatchResponse { accepted_event_ids: ids })
        }
        .boxed()
    }

    fn list_explanations<'a>(&'a self, _emp_sid: i64) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>> {
        async move { Ok(Vec::new()) }.boxed()
    }

    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>> {
        async move {
            self.submitted.lock().unwrap().push(req);
            Ok(())
        }
        .boxed()
    }

    fn get_attendance<'a>(&'a self, _emp_sid: i64) -> BoxFuture<'a, Result<AttendanceSnapshot>> {
        async move {
            Ok(AttendanceSnapshot {
                attendance_status: AttendanceStatus::Working,
                work_start_at: Some(Utc::now()),
                work_end_at: None,
            })
        }
        .boxed()
    }

    fn get_user_info<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<UserInfoSnapshot>> {
        async move {
            Ok(UserInfoSnapshot {
                user: UserInfoUser {
                    employee_id: emp_sid.to_string(),
                    employee_name: "박일일".to_string(),
                    english_name: String::new(),
                    company_id: "11402".to_string(),
                    company_name: "성민".to_string(),
                    team_id: Some("9869".to_string()),
                    team_name: "개발".to_string(),
                    team_template_id: Some("3221".to_string()),
                    team_template_name: "성민".to_string(),
                    position: String::new(),
                    employee_number: String::new(),
                    phone: "01011112222".to_string(),
                    email: "mock@example.com".to_string(),
                    authority: 5,
                    join_date: Some("2020-02-02".to_string()),
                    leave_date: None,
                },
                subscription: UserInfoSubscription {
                    plan_code: "PRO".to_string(),
                    payment_status: "ACTIVE".to_string(),
                    pc_tracking_enabled: true,
                    can_track_time: true,
                    valid_until: Some("2026-12-31".to_string()),
                },
                attendance: UserInfoAttendance {
                    attendance_status: AttendanceStatus::Working,
                    work_start_at: Some(Utc::now()),
                    work_end_at: None,
                },
                polled_at: Utc::now(),
                next_poll_seconds: 3600,
                force_logout: false,
                explanation_types_version: 1_736_654_321,
            })
        }
        .boxed()
    }

    fn get_work_status<'a>(
        &'a self,
        _empsid: i64,
    ) -> BoxFuture<'a, Result<crate::data::dto::work_status_dto::WorkStatusResponseDto>> {
        async move {
            // Mock: 근무중 가정 (result > 0).
            Ok(crate::data::dto::work_status_dto::WorkStatusResponseDto {
                result: 14148240,
                startdt: "2026-05-12 09:00:00".to_string(),
            })
        }
        .boxed()
    }

    fn patch_policy<'a>(
        &'a self,
        req: PolicyPatchRequest,
    ) -> BoxFuture<'a, Result<PolicySnapshot>> {
        async move {
            // Mock: 기본 정책에 patch 적용한 결과 반환. POLICY_VERSION 증가.
            let mut p = Self::fake_policy();
            p.policy_version += 1;
            if let Some(v) = req.patch.idle_threshold_seconds {
                p.effective_idle_threshold_seconds = v;
                p.company_idle_threshold_seconds = Some(v);
            }
            if let Some(v) = req.patch.lunch_start_time {
                p.lunch_start_time = v;
            }
            if let Some(v) = req.patch.lunch_end_time {
                p.lunch_end_time = v;
            }
            if let Some(v) = req.patch.lunch_allowed_minutes {
                p.lunch_allowed_minutes = v;
            }
            if let Some(v) = req.patch.explanation_deadline_hours {
                p.explanation_deadline_hours = v;
            }
            if let Some(v) = req.patch.can_track_time {
                p.can_track_time = v;
            }
            if let Some(v) = req.patch.time_zone_offset_minutes {
                p.time_zone_offset_minutes = v;
            }
            Ok(p)
        }
        .boxed()
    }

    fn create_explanation_type<'a>(
        &'a self,
        req: CreateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>> {
        async move {
            // Mock: code 는 서버 자동 생성 — 임의 형식 echo.
            Ok(ExplanationType {
                exptype_sid: Some(9999),
                code: format!("CUSTOM_MOCK_{}", req.cmpsid),
                label: req.label,
                sort_order: req.sort_order,
                icon: req.icon,
                requires_text: req.requires_text,
                is_system: false,
                is_protected: false,
            })
        }
        .boxed()
    }

    fn update_explanation_type<'a>(
        &'a self,
        sid: i64,
        req: PatchExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>> {
        async move {
            // Mock: 변경된 필드만 반영, 나머지는 임의 시드.
            Ok(ExplanationType {
                exptype_sid: Some(sid),
                code: "MOCK".to_string(),
                label: req.label.unwrap_or_else(|| "Mock 사유".to_string()),
                sort_order: req.sort_order.unwrap_or(999),
                icon: req.icon,
                requires_text: req.requires_text.unwrap_or(false),
                is_system: false,
                is_protected: false,
            })
        }
        .boxed()
    }

    fn deactivate_explanation_type<'a>(
        &'a self,
        _sid: i64,
        _req: DeactivateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<()>> {
        async move { Ok(()) }.boxed()
    }

    fn get_explanation_usage<'a>(
        &'a self,
        _requester_emp_sid: i64,
        _days: u32,
    ) -> BoxFuture<'a, Result<Vec<ExplanationUsageEntry>>> {
        async move {
            Ok(vec![
                ExplanationUsageEntry {
                    code: "MEETING".to_string(),
                    label: "회의".to_string(),
                    count: 23,
                    distinct_users: 7,
                },
                ExplanationUsageEntry {
                    code: "OTHER_WORK".to_string(),
                    label: "기타 업무".to_string(),
                    count: 12,
                    distinct_users: 5,
                },
            ])
        }
        .boxed()
    }

    fn list_explanation_types<'a>(
        &'a self,
        _emp_sid: i64,
    ) -> BoxFuture<'a, Result<ExplanationTypesResponse>> {
        async move {
            // 서버 워커 GET 응답에는 SID 가 포함됨(2026-05-12). mock 도 1부터 enumerate 채워
            // CMS 비활성화 UI 가 표시·전송할 수 있게 한다.
            let types = crate::domain::service::explanation_type_service::system_default_types()
                .into_iter()
                .enumerate()
                .map(|(i, mut t)| {
                    t.exptype_sid = Some((i as i64) + 1);
                    t
                })
                .collect();
            Ok(ExplanationTypesResponse {
                scope: "COMPANY".to_string(),
                scope_keys: ExplanationScopeKeys {
                    cmpsid: 11402,
                    ttmsid: Some(3221),
                    temsid: Some(9869),
                },
                types,
                version: 1_736_654_321,
                seeded: true,
            })
        }
        .boxed()
    }
}
