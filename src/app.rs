//! 백그라운드 작업과 UI 가 공유하는 런타임 상태.
//!
//! - DB 핸들과 설정은 `Arc` 를 통해 모든 모듈이 공유.
//! - 변경 가능 상태(세션, 정책, 출근 상태)는 `RwLock` 로 보호.
//! - UI 갱신용 알림은 `tokio::sync::watch` 채널 사용.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use tokio::runtime::Handle;

use crate::api::ApiClient;
use crate::api::types::{AttendanceStatus, PolicySnapshot, UpdateInfo};
use crate::auth::Session;
use crate::config::AppConfig;
use crate::db::Database;
use crate::device::DeviceInfo;

/// 현재 PC 상태 — UI/heartbeat 가 함께 사용.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcStatus {
    Active,
    Idle,
    Locked,
    AppClosing,
    Offline,
}

impl PcStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PcStatus::Active => "ACTIVE",
            PcStatus::Idle => "IDLE",
            PcStatus::Locked => "LOCKED",
            PcStatus::AppClosing => "APP_CLOSING",
            PcStatus::Offline => "OFFLINE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiveStatus {
    pub pc_status: PcStatus,
    pub last_activity_at: DateTime<Utc>,
    pub idle_seconds: u64,
    pub is_locked: bool,
    pub attendance: AttendanceStatus,
    pub can_track_time: bool,
    pub effective_idle_threshold_seconds: u64,
    pub policy_scope: String,
    pub policy_version: i64,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub last_event_sync_at: Option<DateTime<Utc>>,
    pub last_policy_sync_at: Option<DateTime<Utc>>,
}

impl LiveStatus {
    fn initial(default_idle: u64) -> Self {
        Self {
            pc_status: PcStatus::Active,
            last_activity_at: Utc::now(),
            idle_seconds: 0,
            is_locked: false,
            attendance: AttendanceStatus::Unknown,
            can_track_time: false,
            effective_idle_threshold_seconds: default_idle,
            policy_scope: "DEFAULT".to_string(),
            policy_version: 0,
            last_heartbeat_at: None,
            last_event_sync_at: None,
            last_policy_sync_at: None,
        }
    }
}

pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub device: DeviceInfo,
    pub api: Arc<dyn ApiClient>,
    pub runtime: Handle,
    pub session: RwLock<Option<Session>>,
    pub status: RwLock<LiveStatus>,
    pub policy: RwLock<PolicySnapshot>,
    pub update_info: RwLock<Option<UpdateInfo>>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Database, device: DeviceInfo, runtime: Handle) -> Self {
        let api: Arc<dyn ApiClient> = if config.api.mock_mode {
            Arc::new(crate::api::mock::MockClient::new())
        } else {
            Arc::new(crate::api::client::HttpApiClient::new(
                config.api.base_url.clone(),
                config.api.timeout_seconds,
            ))
        };
        let default_idle = config.policy_defaults.default_idle_threshold_seconds;
        let policy = PolicySnapshot::fallback(default_idle, &config.policy_defaults);
        Self {
            config,
            db,
            device,
            api,
            runtime,
            session: RwLock::new(None),
            status: RwLock::new(LiveStatus::initial(default_idle)),
            policy: RwLock::new(policy),
            update_info: RwLock::new(None),
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.read().map(|s| s.is_some()).unwrap_or(false)
    }

    pub fn can_track_time(&self) -> bool {
        self.status.read().map(|s| s.can_track_time).unwrap_or(false)
    }

    pub fn set_session(&self, sess: Option<Session>) {
        if let Ok(mut w) = self.session.write() {
            *w = sess;
        }
    }

    pub fn snapshot_status(&self) -> LiveStatus {
        self.status.read().unwrap().clone()
    }

    pub fn snapshot_policy(&self) -> PolicySnapshot {
        self.policy.read().unwrap().clone()
    }
}
