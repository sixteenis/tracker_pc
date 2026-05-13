//! ============================================================================
//! monitor::lifecycle — 앱 시작/종료 이벤트.
//! ============================================================================
//!
//! - `record_started` : `APP_STARTED` 이벤트 enqueue.
//! - `record_stopped` : `APP_STOPPED` 이벤트 enqueue (트레이 "종료" 시).
//!
//! NO_PC_RECORD 자동 검출은 heartbeat 제거 시점에 폐기됨. last-seen 의 진실
//! 소스가 서버측 `PCAGT_PRESENCE_LOG` 로 이관됐기 때문에, 단절 구간(NO_PC_RECORD)
//! 생성도 **서버 책임** 이다 (다음 LOGIN/PC_SHUTDOWN 수신 시 직전 이벤트와의
//! gap 으로 segment 생성). 사양: `Review/변경_heartbeat_제거_및_PRESENCE_DB.md`.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;

use crate::app::AppState;
use crate::data::local::events_repo;

/// 앱 시작 직후 호출 (`monitor::spawn_all` 에서). APP_STARTED 이벤트 enqueue.
pub fn record_started(state: &Arc<AppState>) -> Result<()> {
    events_repo::enqueue(
        &state.db,
        "APP_STARTED",
        Utc::now(),
        &serde_json::json!({
            "app_version": state.config.app.app_version,
            "device_id": state.device.device_id,
            "device_name": state.device.device_name,
        }),
    )?;
    Ok(())
}

/// 앱 정상 종료 직전 호출 — APP_STOPPED 이벤트 enqueue.
/// 호출 지점: `ui::PinpleApp::update` 의 트레이 "종료" 처리.
pub fn record_stopped(state: &Arc<AppState>) -> Result<()> {
    events_repo::enqueue(
        &state.db,
        "APP_STOPPED",
        Utc::now(),
        &serde_json::json!({ "device_id": state.device.device_id }),
    )?;
    Ok(())
}
