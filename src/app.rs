//! ============================================================================
//! app.rs — 백그라운드 작업과 UI 가 공유하는 런타임 상태 컨테이너.
//! ============================================================================
//!
//! - 모든 모듈이 `Arc<AppState>` 를 들고 있어 동일한 DB/설정/세션을 봄.
//! - 변경 가능 상태(세션·라이브 상태·정책·업데이트 정보)는 `RwLock` 으로 보호.
//! - 비동기 작업 스폰 핸들(`runtime`) 도 여기에 보관 — UI 가 클릭 콜백에서
//!   `state.runtime.spawn(async ...)` 로 비동기 호출.
//!
//! ── 디자인 노트 ─────────────────────────────────────────────────────────
//! - `RwLock` 보호 영역에서 `await` 하지 않도록 주의 (Tokio Mutex 가 아님).
//!   읽고 즉시 clone 후 lock drop. (sync 모듈 전체에 동일 패턴 적용됨.)
//! - UI 는 1초마다 repaint 하며 `snapshot_*` 메서드로 최신 값을 가져온다.
//!
//! TODO(2차): UI repaint 트리거 채널(`tokio::sync::watch` 또는 `egui_ctx.request_repaint`)
//! 을 도입해서 idle/heartbeat 이벤트 발생 시 즉시 화면 갱신.
//! TODO(2차): 종료 시 `record_stopped` 호출 hook (eframe::App::on_exit) 추가.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use tokio::runtime::Handle;

use crate::api::ApiClient;
use crate::api::types::{AttendanceStatus, PolicySnapshot, UpdateInfo};
use crate::auth::Session;
use crate::config::AppConfig;
use crate::db::Database;
use crate::device::DeviceInfo;

/// 현재 PC 상태 — UI / heartbeat 송신 / 통계 집계가 공통으로 사용.
///
/// 상태 전이는 `monitor::idle_detector` (Active↔Idle), `monitor::session_events`
/// (Locked) 가 담당. `Offline` / `AppClosing` 은 sync 레이어가 설정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcStatus {
    Active,
    Idle,
    Locked,
    AppClosing,
    Offline,
}

impl PcStatus {
    /// 서버 페이로드(`heartbeat`) 에 그대로 사용되는 ASCII 코드.
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

/// 매 5초마다 갱신되는 라이브 상태. heartbeat / UI 양쪽이 참조.
#[derive(Debug, Clone)]
pub struct LiveStatus {
    pub pc_status: PcStatus,
    pub last_activity_at: DateTime<Utc>,
    pub idle_seconds: u64,
    pub is_locked: bool,
    pub attendance: AttendanceStatus,
    /// 요금제 권한 (`subscription.can_track_time && policy.can_track_time`).
    /// false 면 idle 감지/이벤트 enqueue/heartbeat 모두 skip.
    pub can_track_time: bool,
    pub effective_idle_threshold_seconds: u64,
    pub policy_scope: String,
    pub policy_version: i64,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub last_event_sync_at: Option<DateTime<Utc>>,
    pub last_policy_sync_at: Option<DateTime<Utc>>,
}

impl LiveStatus {
    /// 로그인 전 초기값. `can_track_time` 은 보수적으로 false.
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

/// 모든 컴포넌트가 공유하는 단일 상태 컨테이너. `Arc<AppState>` 로 전달.
pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub device: DeviceInfo,
    /// `mock_mode` 설정에 따라 `MockClient` 또는 `HttpApiClient` 로 dispatch.
    pub api: Arc<dyn ApiClient>,
    pub runtime: Handle,
    pub session: RwLock<Option<Session>>,
    pub status: RwLock<LiveStatus>,
    pub policy: RwLock<PolicySnapshot>,
    pub update_info: RwLock<Option<UpdateInfo>>,
}

impl AppState {
    /// 로그인 전 단계의 초기 상태로 새 인스턴스 생성.
    /// `config.api.mock_mode` 에 따라 API 구현체가 자동 선택된다.
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

    /// 활성 세션 보유 여부. UI 라우팅에서 로그인/로그아웃 분기로 사용.
    pub fn is_logged_in(&self) -> bool {
        self.session.read().map(|s| s.is_some()).unwrap_or(false)
    }

    /// 요금제 + 정책 양쪽이 PC 추적을 허용하는지.
    /// false 일 때 `monitor::idle_detector` 와 `sync::heartbeat` 가 자동 skip.
    pub fn can_track_time(&self) -> bool {
        self.status.read().map(|s| s.can_track_time).unwrap_or(false)
    }

    /// 세션 갱신/제거. `auth` 모듈만 사용.
    pub fn set_session(&self, sess: Option<Session>) {
        if let Ok(mut w) = self.session.write() {
            *w = sess;
        }
    }

    /// 라이브 상태 스냅샷 (`await` 가능한 곳에서 안전하게 들고 있을 수 있음).
    pub fn snapshot_status(&self) -> LiveStatus {
        self.status.read().unwrap().clone()
    }

    /// 정책 스냅샷.
    pub fn snapshot_policy(&self) -> PolicySnapshot {
        self.policy.read().unwrap().clone()
    }
}
