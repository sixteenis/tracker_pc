//! ============================================================================
//! data::repository::subscription_repository — 회사 요금제 권한 조회.
//! ============================================================================
//!
//! `/android/check_pay_use.jsp` 를 호출해 PIN+ 권한을 확인한다.
//! 도메인 변환 후 `Subscription` 만 외부에 노출.

use anyhow::{Context, Result};

use crate::app::AppState;
use crate::data::dto::pay_use_dto::CheckPayUseResponseDto;
use crate::domain::model::subscription::Subscription;

/// PIN+ 권한 확인. 호출자(usecase) 가 결과의 `pin_plus_active` 로 진입 분기.
pub async fn fetch(state: &AppState, cmpsid: i64, mbrsid: i64) -> Result<Subscription> {
    let dto = state
        .api
        .check_pay_use(cmpsid, mbrsid)
        .await
        .context("요금제 확인 요청 실패")?;
    Ok(to_subscription(&dto))
}

fn to_subscription(dto: &CheckPayUseResponseDto) -> Subscription {
    Subscription { pin_plus_active: dto.pinpluse, paid_active: dto.payuse }
}
