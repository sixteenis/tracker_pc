//! `keyring` 크레이트로 OS 자격증명 저장소에 refresh token 만 보관.
//!
//! - Windows: Credential Manager (DPAPI 로 자동 암호화).
//! - macOS:   Keychain.
//! - Linux:   Secret Service.
//!
//! 비밀번호와 access_token 은 저장하지 않는다.

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "io.pinple.pcagent";
const REFRESH_USERNAME_PREFIX: &str = "refresh:";

fn entry_for(employee_id: &str) -> Result<Entry> {
    let username = format!("{REFRESH_USERNAME_PREFIX}{employee_id}");
    Entry::new(SERVICE, &username).context("keyring entry 생성 실패")
}

pub fn save_refresh_token(employee_id: &str, token: &str) -> Result<()> {
    let entry = entry_for(employee_id)?;
    entry.set_password(token).context("refresh token 저장 실패")?;
    Ok(())
}

pub fn load_refresh_token(employee_id: &str) -> Result<Option<String>> {
    let entry = entry_for(employee_id)?;
    match entry.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

pub fn clear_refresh_token(employee_id: &str) -> Result<()> {
    let entry = entry_for(employee_id)?;
    match entry.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}
