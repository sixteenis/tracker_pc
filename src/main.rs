//! 핀플 PC 앱 1차 MVP 진입점.
//!
//! - 출근/퇴근은 절대 처리하지 않음 (스마트폰 앱 전용).
//! - PC 사용/미사용 감지 + 근무시간 소명만 담당.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod api;
mod auth;
mod config;
mod db;
mod device;
mod lunch;
mod monitor;
mod notify;
mod sync;
mod ui;
mod util;

use std::sync::Arc;

use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("pinple_pc_agent={level},warn")));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).compact())
        .init();
}

fn main() -> anyhow::Result<()> {
    // 1. 설정 로드 (Mock 모드 여부 등)
    let cfg = config::AppConfig::load()?;
    init_logging(&cfg.logging.level);
    info!(version = %cfg.app.app_version, mock = cfg.api.mock_mode, "핀플 PC 앱 시작");
    info!(
        default_idle_threshold_seconds = cfg.policy_defaults.default_idle_threshold_seconds,
        idle_check_interval_seconds = cfg.intervals.idle_check_interval_seconds,
        heartbeat_interval_seconds = cfg.intervals.heartbeat_interval_seconds,
        "로드된 정책 fallback / 인터벌"
    );

    // 2. 비동기 런타임 — 백그라운드 동기화/감지 루프 전용.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("pinple-bg")
        .build()?;

    // 3. SQLite 초기화 + 마이그레이션
    let db_path = config::AppConfig::data_dir()?.join("pinple.db");
    let db = db::Database::open(&db_path)?;
    db.migrate()?;

    // 4. 디바이스 식별자 (최초 1회 영구 저장)
    let device = device::DeviceInfo::load_or_create(&db)?;

    // 5. 공유 앱 상태
    let state = Arc::new(app::AppState::new(cfg.clone(), db, device, runtime.handle().clone()));

    // 6. 백그라운드 작업 — 감지/heartbeat/배치/정책/업데이트
    monitor::spawn_all(state.clone());
    sync::spawn_all(state.clone());

    // 7. UI 메인 루프 (egui — 자체 이벤트 루프 점유)
    let ui_state = state.clone();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("핀플 PC 에이전트")
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([800.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        "핀플 PC",
        native_options,
        Box::new(move |cc| Ok(Box::new(ui::PinpleApp::new(cc, ui_state)))),
    )
    .map_err(|e| anyhow::anyhow!("UI 실행 실패: {e}"))?;

    info!("핀플 PC 앱 종료");
    Ok(())
}
