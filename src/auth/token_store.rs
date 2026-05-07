//! ============================================================================
//! auth::token_store — refresh_token 을 OS Credential Store 에 영속화.
//! ============================================================================
//!
//! `keyring` 크레이트가 OS 별 백엔드를 자동 선택:
//!   - Windows : Credential Manager (DPAPI 로 자동 암호화)
//!   - macOS   : Keychain
//!   - Linux   : Secret Service (libsecret)
//!
//! 한 PC 에서 여러 사용자가 로그인할 수 있도록 username 에 employee_id 를 prefix.
//!
//! TODO(2차): 운영자 / 보안 검토 후 access_token 도 짧은 유효기간이면 keyring 에
//! 저장하는 방안 고려 (현재는 메모리만 → 앱 재시작 시 매번 refresh 호출).
//! TODO(2차): keyring 백엔드 부재(헤드리스 Linux 등) 대응 fallback — 로컬 파일에
//! age 또는 sodium 으로 암호화 저장.

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "io.pinple.pcagent";
const REFRESH_USERNAME_PREFIX: &str = "refresh:";

/// `(SERVICE, "refresh:<employee_id>")` 키로 keyring entry 생성.
fn entry_for(employee_id: &str) -> Result<Entry> {
    let username = format!("{REFRESH_USERNAME_PREFIX}{employee_id}");
    Entry::new(SERVICE, &username).context("keyring entry 생성 실패")
}

/// 자동로그인 체크 후 호출. 토큰을 OS 저장소에 평문 그대로 넘기지만
/// OS 가 자동으로 암호화한다 (DPAPI / Keychain).
pub fn save_refresh_token(employee_id: &str, token: &str) -> Result<()> {
    let entry = entry_for(employee_id)?;
    entry.set_password(token).context("refresh token 저장 실패")?;
    Ok(())
}

/// 자동로그인 시도 시 호출. 없으면 `Ok(None)` (로그인 화면 표시 신호).
pub fn load_refresh_token(employee_id: &str) -> Result<Option<String>> {
    let entry = entry_for(employee_id)?;
    match entry.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

/// 로그아웃 또는 자동로그인 해제 시 호출. 항목이 이미 없어도 OK 처리.
pub fn clear_refresh_token(employee_id: &str) -> Result<()> {
    let entry = entry_for(employee_id)?;
    match entry.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}
