//! ============================================================================
//! device — 1근로자 1PC 활성 로그인 식별자.
//! ============================================================================
//!
//! - `device_id`: 최초 1회 UUID 생성 후 `settings` 테이블에 영구 저장.
//! - `device_name`: 호스트명 + OS (예: "DESKTOP-ABC (Windows)").
//!
//! 기획서 §9 — 다른 PC 에서 같은 계정으로 로그인 시 서버가 기존 device_id 를
//! 비활성화하므로, 클라이언트는 자기 device_id 만 매 요청에 실어 보내면 된다.
//!
//! TODO(2차): device_name 사용자 변경 UI (현재 호스트명 자동 — "사무실 PC" 같은
//! 별명을 사용자가 설정할 수 있게).
//! TODO(2차): 이전 PC 연결 해제 알림 — 서버 응답의 `displaced_device` 가 채워져
//! 있으면 우측 하단 토스트로 "다른 PC 에서 로그인되어 이 기기 세션을 종료합니다" 표시.

use anyhow::Result;
use uuid::Uuid;

use crate::data::local::{settings_repo, Database};

const KEY_DEVICE_ID: &str = "device_id";
const KEY_DEVICE_NAME: &str = "device_name";

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
}

impl DeviceInfo {
    /// `settings` 테이블의 `device_id`/`device_name` 키를 읽고, 없으면 생성한다.
    /// 같은 PC 의 같은 OS 사용자 프로필에서는 항상 같은 device_id 가 반환된다.
    pub fn load_or_create(db: &Database) -> Result<Self> {
        let device_id = match settings_repo::get(db, KEY_DEVICE_ID)? {
            Some(v) => v,
            None => {
                let v = Uuid::new_v4().to_string();
                settings_repo::set(db, KEY_DEVICE_ID, &v)?;
                v
            }
        };
        let device_name = match settings_repo::get(db, KEY_DEVICE_NAME)? {
            Some(v) => v,
            None => {
                let v = detect_device_name();
                settings_repo::set(db, KEY_DEVICE_NAME, &v)?;
                v
            }
        };
        Ok(Self { device_id, device_name })
    }
}

/// 호스트명을 OS 별 방식으로 읽어 "<호스트> (<OS>)" 로 합친다.
/// TODO(2차): WMI/IOKit 으로 더 정확한 모델명 (예: MacBook Pro / Surface) 조회.
fn detect_device_name() -> String {
    let host = hostname().unwrap_or_else(|| "unknown-host".to_string());
    let os = if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Other"
    };
    format!("{host} ({os})")
}

#[cfg(windows)]
fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

#[cfg(not(windows))]
fn hostname() -> Option<String> {
    std::env::var("HOSTNAME").ok().or_else(|| {
        // BSD/macOS: 보통 `gethostname` 라이브러리가 없을 때 환경변수가 비어있을 수 있음.
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    })
}
