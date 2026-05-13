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
use crate::data::dto::main_info_dto::MainInfoResponseDto;
use crate::data::dto::pay_use_dto::CheckPayUseResponseDto;
use crate::data::dto::*;

pub trait ApiClient: Send + Sync {
    /// GET `/android/check_mbr.jsp` — 이메일 + SHA1 PASS + 디바이스 메타로 로그인.
    /// `LoginResponseDto::is_success()` 가 false 인 경우(`mbrsid <= 0`) 응답은
    /// 정상이지만 인증 실패. 호출자(`auth_repository::login`) 가 분기 처리.
    fn login<'a>(&'a self, req: LoginRequestDto) -> BoxFuture<'a, Result<LoginResponseDto>>;

    /// GET `/android/check_pay_use.jsp?CMPSID=&MBRSID=` — 회사의 PIN+ 사용 권한 확인.
    /// `pinpluse=false` 면 PC Agent 진입 차단 (호출자에서 분기).
    fn check_pay_use<'a>(
        &'a self,
        cmpsid: i64,
        mbrsid: i64,
    ) -> BoxFuture<'a, Result<CheckPayUseResponseDto>>;

    /// GET `/android/u/get_main2.jsp?EMPSID=&CMPSID=&TTMSID=&TEMSID=` — 로그인 후 메인 정보.
    fn get_main_info<'a>(
        &'a self,
        empsid: i64,
        cmpsid: i64,
        ttmsid: i64,
        temsid: i64,
    ) -> BoxFuture<'a, Result<MainInfoResponseDto>>;

    /// GET `/android/u/get_workstatus.jsp?EMPSID=` — 근로자 출퇴근 상태 판별 (2026-05-12 신규).
    /// 응답 `result>0` = 근무중, `result==0` + main_info starttm/endtm 조합으로 미출근/퇴근.
    fn get_work_status<'a>(
        &'a self,
        empsid: i64,
    ) -> BoxFuture<'a, Result<crate::data::dto::work_status_dto::WorkStatusResponseDto>>;

    /// GET /api/pc-agent/policy?emp_sid= — 자리비움 기준/점심 정책 등 재조회.
    /// 서버가 EMPSID 로 회사/팀을 역추적해서 EMPLOYEE>TEAM>COMPANY 우선순위로 계산.
    fn get_policy<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<PolicySnapshot>>;

    /// PATCH /api/pc-agent/policy — 회사 관리자(`Emply.Author >= 5`) 가 정책 부분 업데이트.
    /// 응답은 변경 후 `PolicySnapshot` (GET 과 동일 스키마). 입력 범위·거부 필드는
    /// [[API_명세_핀플_PC_Agent]] §3-1 + `policy_patch_dto::PolicyPatchFields::validate`.
    fn patch_policy<'a>(
        &'a self,
        req: PolicyPatchRequest,
    ) -> BoxFuture<'a, Result<PolicySnapshot>>;

    /// GET /api/pc-agent/update-check — 최신 버전/강제 업데이트 정보.
    /// 인증 헤더 불필요.
    fn update_check<'a>(&'a self, req: UpdateCheckRequest) -> BoxFuture<'a, Result<UpdateInfo>>;

    /// POST /api/pc-agent/events — 의미 있는 이벤트 배치 전송 (1분 주기, 최대 50건).
    /// PRESENCE(LOGIN_SUCCESS/AUTO_LOGIN_SUCCESS/LOGOUT/APP_STOPPED/PC_SHUTDOWN_DETECTED)
    /// 도 같은 채널로 흘러가며, 서버 서비스 레이어가 PCAGT_PRESENCE_LOG 로 매핑한다.
    /// TODO(서버 연동): 인증 파라미터 확정.
    fn send_events<'a>(
        &'a self,
        batch: EventsBatch,
    ) -> BoxFuture<'a, Result<EventsBatchResponse>>;

    /// GET /api/pc-agent/worktime-explanations?emp_sid= — 서버 측 자리비움/소명 목록.
    fn list_explanations<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<Vec<RemoteExplanation>>>;

    /// POST /api/pc-agent/worktime-explanations — 사용자가 입력한 소명 제출.
    /// `ExplanationSubmit.employee_id` 로 EMPSID 전달.
    fn submit_explanation<'a>(&'a self, req: ExplanationSubmit) -> BoxFuture<'a, Result<()>>;

    /// GET /api/pc-agent/attendance-status?emp_sid= — 오늘 출근 상태 조회.
    /// (deprecated — `get_user_info` 로 대체. 신규 코드는 사용 금지)
    fn get_attendance<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<AttendanceSnapshot>>;

    /// GET /api/pc-agent/user-info?emp_sid= — user/subscription/attendance 통합 조회.
    /// 로그인 직후 1회 + 응답의 `next_poll_seconds` 간격으로 폴링.
    fn get_user_info<'a>(&'a self, emp_sid: i64) -> BoxFuture<'a, Result<UserInfoSnapshot>>;

    /// GET /api/pc-agent/explanation-types?emp_sid= — 회사별 활성 소명사유 동적 목록.
    /// user-info 응답의 `explanation_types_version` 이 클라 캐시와 다를 때만 호출.
    /// 응답은 `settings` KV (`explanation_types_<cmpsid>` + `*_version_<cmpsid>`) 에 캐시.
    fn list_explanation_types<'a>(
        &'a self,
        emp_sid: i64,
    ) -> BoxFuture<'a, Result<ExplanationTypesResponse>>;

    // ── 회사 관리자(`Emply.Author>=5`) CMS CRUD (Phase 2, 2026-05-12) ──
    // 현재 클라는 dev 회사설정 옆 "사유 관리(테스트)" 탭에서만 호출 — release 빌드 제외.

    /// POST /api/cms/pc-agent/explanation-types — 회사 새 사유 추가.
    fn create_explanation_type<'a>(
        &'a self,
        req: CreateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>>;

    /// PATCH /api/cms/pc-agent/explanation-types/:sid — LABEL/SORT/ICON/REQUIRES_TEXT 부분 수정.
    fn update_explanation_type<'a>(
        &'a self,
        sid: i64,
        req: PatchExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<ExplanationType>>;

    /// PATCH /api/cms/pc-agent/explanation-types/:sid/deactivate — soft delete.
    /// 서버가 활성 셋 ≥ 1 가드 — 마지막 활성 row 비활성 시도 시 `409 AT_LEAST_ONE_REQUIRED`.
    fn deactivate_explanation_type<'a>(
        &'a self,
        sid: i64,
        req: DeactivateExplanationTypeRequest,
    ) -> BoxFuture<'a, Result<()>>;

    /// GET /api/cms/pc-agent/explanation-types/usage?days=&emp_sid= — 사용 통계 (count + distinct_users).
    fn get_explanation_usage<'a>(
        &'a self,
        requester_emp_sid: i64,
        days: u32,
    ) -> BoxFuture<'a, Result<Vec<ExplanationUsageEntry>>>;
}
