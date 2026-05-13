//! ============================================================================
//! data::repository::main_info_repository — 메인 정보 조회 + 도메인 변환.
//! ============================================================================
//!
//! `/android/u/get_main2.jsp` 호출 → `MainInfoResponseDto` → `MainInfo`.
//! DTO 는 본 모듈을 벗어나지 않는다.

use anyhow::{Context, Result};

use crate::app::AppState;
use crate::data::dto::main_info_dto::MainInfoResponseDto;
use crate::domain::model::main_info::MainInfo;

/// 로그인 직후 / 자동로그인 직후 호출. usecase 가 호출자.
pub async fn fetch(
    state: &AppState,
    empsid: i64,
    cmpsid: i64,
    ttmsid: i64,
    temsid: i64,
) -> Result<MainInfo> {
    let dto = state
        .api
        .get_main_info(empsid, cmpsid, ttmsid, temsid)
        .await
        .context("메인 정보 요청 실패")?;
    Ok(to_main_info(&dto))
}

fn to_main_info(dto: &MainInfoResponseDto) -> MainInfo {
    MainInfo {
        start_time: dto.starttm.clone(),
        end_time: dto.endtm.clone(),
        join_date: dto.joindt.clone(),
        remaining_annual_minutes: dto.anual,
        work_minutes: dto.workmin,
        add_minutes: dto.addmin,
        used_minutes: dto.usemin,
        unread_message_count: dto.msgcnt,
        deduct_on_late: dto.anualddctn1 != 0,
        deduct_on_early_leave: dto.anualddctn2 != 0,
        deduct_on_outing: dto.anualddctn3 != 0,
        annual_by_join_date: dto.st_anual != 0,
        daily_annual_by_join_date: dto.st_d_anual != 0,
        use_break_time: dto.brk_time != 0,
        use_schedule: dto.schdl != 0,
        auto_checkout_mode: dto.cmt_lt,
        commute_notify: dto.cmtnoti != 0,
        work_52h_unit: dto.wk52h,
    }
}
