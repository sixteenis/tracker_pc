//! ============================================================================
//! sync::user_info_sync — V1 check_pay_use + V2 user-info + V1 get_main2 통합 폴링.
//! ============================================================================
//!
//! 기획서 §10 + 부록 C. heartbeat 제거 결정(2026-05-12) 이후 본 모듈이
//! 다음 책임을 모두 흡수:
//!   - **요금제 (`pinpluse` 단일 결정자)** — `check_pay_use.jsp` 호출 후
//!     `state.status.can_track_time` + `subscription_service` 양쪽 갱신.
//!     pinpluse 가 false → true 로 변경된 경우도 즉시 반영(추적 자동 재개).
//!   - **유저/출근 상태 (V2)** — `/api/pc-agent/user-info` 호출. `force_logout=true`
//!     수신 시 `LOGOUT/FORCE_LOGOUT` 이벤트 enqueue 후 즉시 `user_service::logout`.
//!     `attendance` 는 `idle_detector` 엔진 게이트의 진실 소스.
//!   - **출퇴근 시각 (V1)** — `get_main2.jsp` 호출. UI 메인화면의 출근 표시는
//!     `main_info.start_time` / `end_time` 으로 판별 (미출근/근무중/퇴근).
//!
//! 적응형 주기:
//!   - 로그인 직후 1회 + 응답의 `next_poll_seconds` 간격으로 반복.
//!   - `attendance_status == "WORKING"` → 1시간 (3600초)
//!   - 그 외 (`BEFORE_WORK`/`AFTER_WORK`/`OUTING`/`LEAVE`/`BUSINESS_TRIP`/`UNKNOWN`) → 5분 (300초)
//!
//! `heartbeat` 와 `attendance_sync` 의 책임을 본 모듈이 모두 이어받았다.
//! 두 모듈 코드는 호환 스텁만 남아있거나(`attendance_sync`) 삭제됨(`heartbeat`).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::app::AppState;
use crate::data::repository::main_info_repository;
use crate::domain::model::subscription::Subscription;
use crate::domain::service::{
    explanation_type_service, main_info_service, subscription_service, user_service,
    work_status_service,
};

/// 서버 응답 누락 시 사용할 안전 기본값.
const FALLBACK_NEXT_POLL_SECONDS: u64 = 300;
/// 최소 / 최대 클램프 — 운영자가 너무 짧거나 길게 설정한 경우 방어.
const MIN_NEXT_POLL_SECONDS: u64 = 60;
const MAX_NEXT_POLL_SECONDS: u64 = 6 * 3600;

/// 메인 user-info 폴링 루프.
pub async fn run(state: Arc<AppState>) {
    // 로그인 전에는 짧게 슬립 후 재확인 (자동로그인 끝나기를 기다림).
    let mut next_poll_secs: u64 = FALLBACK_NEXT_POLL_SECONDS;

    // 초기 1회는 짧게 대기 (UI / 자동로그인 정착 시간 확보)
    tokio::time::sleep(Duration::from_secs(2)).await;

    loop {
        let maybe_session = state.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => {
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        // 1) check_pay_use (V1) — 요금제 권한 단일 결정자 (기획 §부록 B).
        //    pinpluse 변화 즉시 반영을 위해 status.can_track_time 뿐 아니라
        //    subscription_service 캐시까지 함께 갱신한다 (idle_detector / status_view
        //    가 subscription_service::pin_plus_active() 로 분기하므로 필수).
        match state.api.check_pay_use(session.company_id, session.member_id).await {
            Ok(pay) => {
                if let Ok(mut s) = state.status.write() {
                    s.can_track_time = pay.pinpluse;
                }
                subscription_service::set(Subscription {
                    pin_plus_active: pay.pinpluse,
                    paid_active: pay.payuse,
                });
                info!(pinpluse = pay.pinpluse, "check_pay_use 갱신");
            }
            Err(e) => {
                warn!(error = %e, "check_pay_use 조회 실패 — 직전 값 유지");
            }
        }

        // 2) user-info (V2) — 유저 / 출근 상태 / force_logout 신호 / explanation_types_version
        match state.api.get_user_info(session.employee_id).await {
            Ok(snap) => {
                // force_logout — 인증 무효화(이메일/비번 변경, 퇴사 등). LOGOUT/FORCE_LOGOUT
                // 이벤트 + 세션 해제. 인증 실패 (자동로그인 단계) 와 명확히 구분되는 케이스.
                if snap.force_logout {
                    warn!("user-info: force_logout 수신 — 세션 해제");
                    let _ = user_service::logout(&state, user_service::LogoutReason::ForceLogout);
                    crate::ui::explanation_list_view::clear_cache();
                    next_poll_secs = FALLBACK_NEXT_POLL_SECONDS;
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    continue;
                }

                // 상태 갱신 — attendance(엔진 게이트 진실 소스). can_track_time 은
                // §1 check_pay_use 결과만 신뢰.
                if let Ok(mut s) = state.status.write() {
                    s.attendance = snap.attendance.attendance_status;
                    s.last_user_info_sync_at = Some(Utc::now());
                }

                // 다음 폴링 주기 (클램프)
                next_poll_secs = snap
                    .next_poll_seconds
                    .clamp(MIN_NEXT_POLL_SECONDS, MAX_NEXT_POLL_SECONDS);

                info!(
                    attendance = ?snap.attendance.attendance_status,
                    plan = %snap.subscription.plan_code,
                    next_poll_seconds = next_poll_secs,
                    "user-info 갱신 완료"
                );

                // 2.1) `/explanation-types` 무조건 재호출 — 디스크 캐시 폐기 정책
                //      (2026-05-12 사용자 결정). 같은 폴링 사이클에 묶어 별도 task 없이 갱신.
                match state.api.list_explanation_types(session.employee_id).await {
                    Ok(resp) => {
                        explanation_type_service::store_response(&resp);
                        info!(
                            version = resp.version,
                            count = resp.types.len(),
                            seeded = resp.seeded,
                            "explanation_types 갱신"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "explanation_types 조회 실패 — 직전 메모리/fallback 유지")
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "user-info 조회 실패 — 다음 시도까지 fallback 대기");
                next_poll_secs = FALLBACK_NEXT_POLL_SECONDS;
            }
        }

        // 2.5) get_workstatus (V1) — 출퇴근 판별 단일 진실 소스 (2026-05-12 도입).
        //      result>0 = 근무중, result==0 + main_info starttm/endtm 으로 미출근/퇴근 구분.
        //      [[T-20260512-XX_workstatus_API_도입]] (생성 예정 백엔드/기획 티켓 참조)
        match state.api.get_work_status(session.employee_id).await {
            Ok(ws) => {
                work_status_service::set_result(ws.result);
                info!(result = ws.result, "get_workstatus 갱신");
            }
            Err(e) => {
                warn!(error = %e, "get_workstatus 조회 실패 — 직전 값 유지");
            }
        }

        // 3) get_main2 (V1) — 메인화면 출퇴근 표시용 starttm/endtm 갱신.
        //    UI 가 main_info_service::current() 의 start_time / end_time 으로
        //    "미출근/근무중/퇴근" 을 판별한다. (엔진의 자리비움 판단은 별개로
        //    §2 user-info 의 attendance_status 를 사용.)
        let team_id = session.team_id.unwrap_or(0);
        match main_info_repository::fetch(
            &state,
            session.employee_id,
            session.company_id,
            session.team_template_id,
            team_id,
        )
        .await
        {
            Ok(info) => {
                let start_set = !info.start_time.is_empty();
                let end_set = !info.end_time.is_empty();
                main_info_service::set(info);
                info!(start_set, end_set, "main_info(get_main2) 갱신");
            }
            Err(e) => {
                warn!(error = %e, "get_main2 조회 실패 — 직전 값 유지");
            }
        }

        tokio::time::sleep(Duration::from_secs(next_poll_secs)).await;
    }
}
