//! ============================================================================
//! main.rs — 핀플 PC 앱 1차 MVP 진입점.
//! ============================================================================
//!
//! 부팅 순서:
//!   1) 설정 로드 (`config/default.toml` + 사용자 override + env)
//!   2) tokio 멀티스레드 런타임 생성 (UI 와 별도)
//!   3) SQLite 열고 마이그레이션
//!   4) device_id / device_name 영속화 (최초 1회 UUID)
//!   5) `AppState` 공유 상태 인스턴스 생성 (Arc)
//!   6) 백그라운드 task 스폰 (감지/heartbeat/배치/정책/업데이트/출근)
//!   7) eframe 메인 루프 점유 — 종료 시 함수 반환
//!
//! ── 설계 원칙 ───────────────────────────────────────────────────────────
//! - 출근/퇴근은 절대 PC 앱이 처리하지 않음. 스마트폰 앱 전용.
//! - PC 앱은 사용/미사용 감지 + 근무시간 소명만 담당.
//! - 비밀번호는 메모리에서만 사용 후 즉시 drop. 토큰만 OS Credential Store 저장.

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

/// tracing 로거를 한 번 초기화한다.
///
/// - 환경변수 `RUST_LOG` 가 있으면 우선.
/// - 없으면 `pinple_pc_agent={level},warn` (자기 크레이트만 상세, 외부 의존성은 warn 이상).
///
/// TODO(2차): 파일 회전 로그 추가 (현재 stdout 만). `tracing-appender` 의
/// `rolling::daily` 를 사용해 `data_dir/logs/agent-YYYY-MM-DD.log` 형태로 저장하면
/// Windows 에서도 디버깅 편의가 좋아진다.
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
    // TODO(2차): 트레이 아이콘 통합. 현재 `Cargo.toml` 의 `tray-icon` 의존성은 주석.
    // egui/winit 이벤트 루프 안에서 tray 메시지를 받으려면 별도 thread + IPC 채널 필요.
    // TODO(2차): 닫기(X) 클릭 시 `hide_to_tray_on_close = true` 면 종료 대신 트레이로 최소화.
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
