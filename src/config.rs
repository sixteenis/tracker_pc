//! 설정 로드 — `config/default.toml` 및 사용자 설정 디렉토리 병합.
//!
//! 우선순위 (낮음 → 높음):
//!   1. 컴파일에 포함된 `config/default.toml`
//!   2. OS 사용자 설정 디렉토리의 `config.toml` (존재 시)
//!   3. 환경변수 `PINPLE_API_BASE_URL`, `PINPLE_MOCK_MODE`

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const DEFAULT_TOML: &str = include_str!("../config/default.toml");

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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Logging {
    pub level: String,
}

impl AppConfig {
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

    pub fn user_config_path() -> Option<PathBuf> {
        ProjectDirs::from("io", "Pinple", "PCAgent").map(|d| d.config_dir().join("config.toml"))
    }

    pub fn data_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("io", "Pinple", "PCAgent")
            .context("OS 사용자 데이터 디렉토리 확인 실패")?;
        let dir = dirs.data_dir().to_path_buf();
        std::fs::create_dir_all(&dir).ok();
        Ok(dir)
    }
}
