//! ============================================================================
//! db::auth_repo — 로그인 식별 정보 저장소 (`auth` 테이블).
//! ============================================================================
//!
//! 비밀번호 / access_token / refresh_token 은 **이 테이블에 저장되지 않는다**.
//! refresh_token 은 OS Credential Store(`auth::token_store`) 에, access_token 은
//! 메모리(`auth::Session`) 에만 보관.
//!
//! 1근로자 1PC 정책이라 `auth` 테이블은 항상 0~1행. `upsert` 는 기존 row 를
//! 모두 지우고 새로 insert 한다.

use anyhow::Result;
use chrono::Utc;

use super::Database;

#[derive(Debug, Clone)]
pub struct AuthRow {
    pub company_id: String,
    pub employee_id: String,
    pub employee_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub device_id: String,
    pub device_name: String,
    pub auto_login: bool,
}

/// 로그인 직후 호출. 기존 행을 모두 지우고 새 식별 정보를 기록한다.
pub fn upsert(db: &Database, row: &AuthRow) -> Result<()> {
    let conn = db.lock();
    // 단일 사용자 보장 — 기존 row 모두 제거 후 새로 삽입.
    conn.execute("DELETE FROM auth", [])?;
    conn.execute(
        "INSERT INTO auth(company_id, employee_id, employee_name, team_id, team_name,
                          device_id, device_name, auto_login, last_login_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            row.company_id,
            row.employee_id,
            row.employee_name,
            row.team_id,
            row.team_name,
            row.device_id,
            row.device_name,
            row.auto_login as i32,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// 자동로그인 시도 시 호출 — 마지막 로그인한 사용자의 식별 정보를 가져온다.
pub fn get(db: &Database) -> Result<Option<AuthRow>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(
        "SELECT company_id, employee_id, employee_name, team_id, team_name,
                device_id, device_name, auto_login FROM auth LIMIT 1",
    )?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        Ok(Some(AuthRow {
            company_id: row.get(0)?,
            employee_id: row.get(1)?,
            employee_name: row.get(2)?,
            team_id: row.get(3)?,
            team_name: row.get(4)?,
            device_id: row.get(5)?,
            device_name: row.get(6)?,
            auto_login: row.get::<_, i32>(7)? != 0,
        }))
    } else {
        Ok(None)
    }
}

/// 로그아웃 시 호출. 토큰 정리는 `auth::token_store::clear_refresh_token` 따로.
pub fn clear(db: &Database) -> Result<()> {
    let conn = db.lock();
    conn.execute("DELETE FROM auth", [])?;
    Ok(())
}
