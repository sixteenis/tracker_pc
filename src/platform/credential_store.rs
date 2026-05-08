//! ============================================================================
//! auth::credential_store — 자동로그인용 EMAIL + SHA1 PASS 저장소.
//! ============================================================================
//!
//! 변경: 토큰 기반(refresh_token) 에서 자격증명 기반으로 전환됨.
//! 평문 비밀번호는 절대 저장되지 않는다. 클라이언트가 SHA-1 해시로 변환한 PASS
//! (40자 hex) 만 OS Credential Store 에 저장한다.
//!
//! 백엔드 (`keyring` crate 자동 선택):
//!   - Windows : Credential Manager (DPAPI)
//!   - macOS   : Keychain
//!   - Linux   : Secret Service (libsecret)
//!
//! 저장 형태: 단일 entry `(KEYRING_SERVICE, "credentials")`,
//! value = JSON `{"email": "...", "password_sha1": "..."}`.
//!
//! 자동로그인 미사용 시(체크박스 해제) 호출자는 `clear()` 로 삭제한다.

use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::constants;

const USERNAME: &str = "credentials";

/// keyring 에 저장되는 JSON 페이로드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub email: String,
    /// 평문이 아닌 SHA-1 해시값 (40자 hex). 서버 query 에 그대로 사용.
    pub password_sha1: String,
}

fn entry() -> Result<Entry> {
    Entry::new(constants::KEYRING_SERVICE, USERNAME).context("keyring entry 생성 실패")
}

/// 자동로그인 체크 후 호출. JSON 직렬화해서 OS 저장소에 평문으로 넘기지만
/// OS 가 자체 암호화한다.
pub fn save(creds: &StoredCredentials) -> Result<()> {
    let entry = entry()?;
    let json = serde_json::to_string(creds).context("credentials 직렬화 실패")?;
    entry.set_password(&json).context("credentials 저장 실패")?;
    Ok(())
}

/// 자동로그인 시도 시 호출. 없으면 `Ok(None)` (로그인 화면 표시 신호).
/// 손상된 JSON 이면 Err — 호출자가 무시하고 수동 로그인으로 폴백 가능.
pub fn load() -> Result<Option<StoredCredentials>> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(json) => {
            let creds: StoredCredentials =
                serde_json::from_str(&json).context("credentials 역직렬화 실패")?;
            Ok(Some(creds))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

/// 로그아웃 또는 자동로그인 해제 시 호출. 항목이 이미 없어도 OK 처리.
pub fn clear() -> Result<()> {
    let entry = entry()?;
    match entry.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}
