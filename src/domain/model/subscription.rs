//! ============================================================================
//! domain::model::subscription — 회사 요금제/권한 도메인 모델.
//! ============================================================================
//!
//! `data::dto::pay_use_dto::CheckPayUseResponseDto` 의 wire 약어를 의미 있는
//! 이름으로 변환한 결과. 본 PC Agent 는 `pin_plus_active` 한 값만으로 진입
//! 가능 여부를 판단한다.

#[derive(Debug, Clone, Copy)]
pub struct Subscription {
    /// PIN+ 사용 권한 (서버 `pinpluse`). false 면 본 프로그램 진입 차단.
    pub pin_plus_active: bool,
    /// 결제 사용 여부 (서버 `payuse`). 향후 안내 메시지용.
    pub paid_active: bool,
}
