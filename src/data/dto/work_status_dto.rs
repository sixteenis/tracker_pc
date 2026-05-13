//! ============================================================================
//! data::dto::work_status_dto — V1 `/android/u/get_workstatus.jsp` 응답 DTO.
//! ============================================================================
//!
//! 2026-05-12 도입. 근로자의 출퇴근 판별 단일 진실 소스 — `main_info` 의
//! starttm/endtm 조합과 함께 사용해 미출근/근무중/퇴근을 결정한다.
//!
//! ── 판별 규칙 (사용자 결정 2026-05-12) ────────────────────────────
//!   - `result > 0`                                                    → **근무중**
//!   - `result == 0` AND `main_info.starttm` 비거나 "00:00"
//!                  AND `main_info.endtm`   비거나 "00:00"             → **미출근**
//!   - `result == 0` AND (starttm 또는 endtm 값이 있고 "00:00" 아님)   → **퇴근**
//!
//! `startdt` 는 출근 시점의 RFC 비슷한 문자열("YYYY-MM-DD HH:MM:SS").
//! 미출근일 땐 공백 패딩(`"                   "`)으로 응답 — `trim()` 후 비어있으면
//! 미설정으로 본다.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkStatusResponseDto {
    /// 0 = 미출근/퇴근, >0 = 근무중 (구체 의미는 백엔드 명세 확정 후).
    #[serde(default)]
    pub result: i64,
    /// 출근 시각 ("YYYY-MM-DD HH:MM:SS"). 미출근일 때 공백 패딩 응답 — `trim()` 후
    /// 비어있으면 미설정.
    #[serde(default)]
    pub startdt: String,
}
