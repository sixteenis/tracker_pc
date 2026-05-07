//! 디바이스 식별자 — 1근로자 1PC 정책에 사용.
//!
//! - `device_id`: 최초 1회 UUID 생성 후 `settings` 테이블에 영구 저장.
//! - `device_name`: 호스트명 + OS (예: "DESKTOP-ABC (Windows)").
//!
//! 기획서 §9 참고. 다른 PC 에서 같은 계정으로 로그인 시 서버가 기존 device_id 를
//! 비활성화하므로, 클라이언트는 그저 자기 device_id 를 보내기만 하면 된다.

use anyhow::Result;
use uuid::Uuid;

use crate::db::{settings_repo, Database};

const KEY_DEVICE_ID: &str = "device_id";
const KEY_DEVICE_NAME: &str = "device_name";

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
}

impl DeviceInfo {
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
