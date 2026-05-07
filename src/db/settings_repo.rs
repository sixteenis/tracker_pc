//! `settings` (key/value) 단순 저장소 — 정책 캐시, 마지막 동기화 시각 등.

use anyhow::Result;
use chrono::Utc;

use super::Database;

pub fn get(db: &Database, key: &str) -> Result<Option<String>> {
    let conn = db.lock();
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get::<_, String>(0)?))
    } else {
        Ok(None)
    }
}

pub fn set(db: &Database, key: &str, value: &str) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![key, value, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
