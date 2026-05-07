//! 로그인 식별 정보. 토큰은 OS Credential Store 에 저장하며 여기에는 들어오지 않음.

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

pub fn clear(db: &Database) -> Result<()> {
    let conn = db.lock();
    conn.execute("DELETE FROM auth", [])?;
    Ok(())
}
