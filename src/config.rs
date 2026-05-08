//! ============================================================================
//! config.rs — 앱 설정 로더 + 전역 접근자.
//! ============================================================================
//!
//! 우선순위 (낮음 → 높음):
//!   1. 컴파일에 포함된 `config/default.toml`
//!   2. OS 사용자 설정 디렉토리의 `config.toml` (존재 시)
//!   3. 환경변수 `PINPLE_API_BASE_URL`, `PINPLE_MOCK_MODE`
//!
//! ── 접근 방법 ──────────────────────────────────────────────────────────
//! 1) `state.config.*` — `Arc<AppState>` 를 들고 있는 곳 (대부분의 모듈).
//! 2) `AppConfig::global().*` — `state` 가 닿지 않는 정적 컨텍스트
//!    (예: 매크로 안, `Default` impl, 헬퍼 함수). `main.rs` 에서 한 번
//!    `AppConfig::init_global` 을 호출한 뒤에만 사용 가능.
//!
//! 모든 모듈은 `AppConfig` 를 read-only 로 공유한다. 런타임 변경은 정책 스냅샷
//! (`AppState::policy`) 으로만 가능 — 설정 자체를 수정해서는 안 됨.
//!
//! TODO(2차): 설정 화면(`ui/settings_view`) 의 토글들을 `config.toml` 에 영속화.
//! 현재 토글은 메모리(`SettingsUi`) 에만 살아있고 재시작 시 초기화됨.
//! TODO(2차): 회사별 환경(staging/prod) 자동 전환을 위한 `PINPLE_ENV` 환경변수 지원.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::constants;

const DEFAULT_TOML: &str = include_str!("../config/default.toml");

/// 프로세스당 한 번 채워지는 전역 설정. `AppConfig::init_global` 로만 채워지며
/// 이후엔 read-only.
static GLOBAL: OnceCell<AppConfig> = OnceCell::new();

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub api: ApiConfig,
    pub app: AppMeta,
    pub intervals: Intervals,
    pub policy_defaults: PolicyDefaults,
    pub logging: Logging,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
    pub mock_mode: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppMeta {
    pub app_version: String,
    pub hide_to_tray_on_close: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Intervals {
    pub idle_check_interval_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub event_batch_interval_seconds: u64,
    pub policy_check_interval_seconds: u64,
    pub update_check_interval_seconds: u64,
    pub max_events_per_batch: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolicyDefaults {
    pub default_idle_threshold_seconds: u64,
    pub default_lunch_start_time: String, // "HH:MM"
    pub default_lunch_end_time: String,
    pub default_lunch_allowed_minutes: u32,
    /// 서버 정책 미수신 시 자리비움 소명 마감 시간 fallback (시간 단위).
    pub default_explanation_deadline_hours: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Logging {
    pub level: String,
}

impl AppConfig {
    /// 우선순위 합쳐 최종 설정을 반환.
    /// 사용자 설정 파일이 없으면 컴파일 기본값 + env 오버라이드만 적용.
    pub fn load() -> Result<Self> {
        let mut cfg: AppConfig =
            toml::from_str(DEFAULT_TOML).context("기본 설정 파싱 실패")?;

        if let Some(user_path) = Self::user_config_path() {
            if user_path.exists() {
                let txt = std::fs::read_to_string(&user_path)
                    .with_context(|| format!("사용자 설정 읽기 실패: {}", user_path.display()))?;
                cfg = toml::from_str(&txt).context("사용자 설정 파싱 실패")?;
            }
        }

        if let Ok(v) = std::env::var("PINPLE_API_BASE_URL") {
            cfg.api.base_url = v;
        }
        if let Ok(v) = std::env::var("PINPLE_MOCK_MODE") {
            cfg.api.mock_mode = matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
        }
        Ok(cfg)
    }

    /// 사용자별 `config.toml` 의 절대 경로 (있을 수도, 없을 수도).
    /// Windows: `%APPDATA%\Pinple\PCAgent\config.toml`
    /// macOS:   `~/Library/Application Support/Pinple/PCAgent/config.toml`
    pub fn user_config_path() -> Option<PathBuf> {
        ProjectDirs::from(constants::APP_QUALIFIER, constants::APP_ORG, constants::APP_NAME)
            .map(|d| d.config_dir().join("config.toml"))
    }

    /// SQLite DB 와 로그를 저장하는 사용자별 데이터 디렉토리.
    /// 없으면 자동 생성.
    pub fn data_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from(
            constants::APP_QUALIFIER,
            constants::APP_ORG,
            constants::APP_NAME,
        )
        .context("OS 사용자 데이터 디렉토리 확인 실패")?;
        let dir = dirs.data_dir().to_path_buf();
        std::fs::create_dir_all(&dir).ok();
        Ok(dir)
    }

    /// 프로세스 시작 시 한 번 호출 — 전역 설정 슬롯 채우기.
    /// 두 번째 호출은 무시 (테스트 동시 실행 등).
    pub fn init_global(cfg: AppConfig) -> &'static AppConfig {
        let _ = GLOBAL.set(cfg);
        GLOBAL.get().expect("AppConfig::init_global 호출 직후엔 항상 채워져 있어야 함")
    }

    /// 전역 설정 read-only 참조.
    /// `init_global` 이전 호출 시 panic — 부팅 순서가 보장되는 main.rs 흐름을 깨지 마세요.
    pub fn global() -> &'static AppConfig {
        GLOBAL.get().expect(
            "AppConfig 가 아직 초기화되지 않았습니다 — main.rs 의 `AppConfig::init_global(cfg)` 호출 이후에만 접근 가능",
        )
    }

    /// 테스트 등에서 안전하게 시도. 미초기화면 None.
    pub fn try_global() -> Option<&'static AppConfig> {
        GLOBAL.get()
    }
}
