//! ============================================================================
//! data::dto::pay_use_dto — `/android/check_pay_use.jsp` 응답 DTO.
//! ============================================================================
//!
//! 본 PC Agent 가 실제로 보는 값은 `pinpluse` 한 항목뿐이지만, 향후 요금제
//! 만료 안내(`paymentList`, `DDay`) 등을 추가하려면 여기 필드를 늘리면 된다.
//! data 레이어 밖으로는 노출하지 않는다 (도메인 변환은 repository).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CheckPayUseResponseDto {
    /// PIN+ 사용 가능 여부 — 본 프로그램 진입 가능 여부 게이트.
    #[serde(default)]
    pub pinpluse: bool,
    /// 결제 사용 여부 (현재 미사용, 향후 안내용).
    #[serde(default)]
    pub payuse: bool,
}
