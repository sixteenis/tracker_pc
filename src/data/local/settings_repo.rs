//! ============================================================================
//! db::settings_repo — `settings` 테이블 (단순 key/value 저장소).
//! ============================================================================
//!
//! 사용처:
//!   - `device_id`, `device_name` (기기 식별)
//!   - `last_heartbeat_at` (앱 비정상 종료 시 NO_PC_RECORD 계산 기준)
//!
//! 가벼운 메타데이터만 들어가야 하며, 큰 데이터(이벤트/segment) 는 전용 테이블 사용.
//!
//! TODO(2차): 사용자 환경설정(자동시작/알림/트레이) 토글값을 여기에 영속화.
//! 현재는 `ui::settings_view` 메모리에서만 살아있다.

use anyhow::Result;
use chrono::Utc;

use super::Database;

/// 키로 값 조회. 없으면 `Ok(None)`.
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

/// 키/값 upsert. 같은 키가 있으면 덮어쓴다.
pub fn set(db: &Database, key: &str, value: &str) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![key, value, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
