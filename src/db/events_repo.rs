//! 서버 전송 대기 이벤트 큐 (`local_events`).
//! - PENDING → 전송 시도 → SUCCESS / FAILED.
//! - `event_id` (UUID) 기준 중복 방지.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use super::Database;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Pending,
    Success,
    Failed,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Pending => "PENDING",
            SyncStatus::Success => "SUCCESS",
            SyncStatus::Failed => "FAILED",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "SUCCESS" => Self::Success,
            "FAILED" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventRow {
    pub id: i64,
    pub event_id: String,
    pub event_type: String,
    pub event_time: DateTime<Utc>,
    pub payload_json: String,
    pub sync_status: SyncStatus,
    pub retry_count: i32,
}

/// 새 이벤트 enqueue. event_id 충돌 시 무시.
pub fn enqueue(
    db: &Database,
    event_type: &str,
    event_time: DateTime<Utc>,
    payload: &serde_json::Value,
) -> Result<String> {
    let event_id = Uuid::new_v4().to_string();
    let conn = db.lock();
    conn.execute(
        "INSERT OR IGNORE INTO local_events(event_id, event_type, event_time, payload_json,
            sync_status, retry_count, created_at)
         VALUES(?1,?2,?3,?4,'PENDING',0,?5)",
        params![
            event_id,
            event_type,
            event_time.to_rfc3339(),
            payload.to_string(),
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(event_id)
}

pub fn pending_batch(db: &Database, limit: u32) -> Result<Vec<EventRow>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(
        "SELECT id, event_id, event_type, event_time, payload_json, sync_status, retry_count
         FROM local_events
         WHERE sync_status IN ('PENDING','FAILED')
         ORDER BY id ASC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            let event_time_str: String = r.get(3)?;
            let event_time = DateTime::parse_from_rfc3339(&event_time_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(EventRow {
                id: r.get(0)?,
                event_id: r.get(1)?,
                event_type: r.get(2)?,
                event_time,
                payload_json: r.get(4)?,
                sync_status: SyncStatus::parse(&r.get::<_, String>(5)?),
                retry_count: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn mark_success(db: &Database, event_ids: &[String]) -> Result<()> {
    if event_ids.is_empty() {
        return Ok(());
    }
    let conn = db.lock();
    let now = Utc::now().to_rfc3339();
    let placeholders = std::iter::repeat("?").take(event_ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE local_events SET sync_status='SUCCESS', synced_at=?1
         WHERE event_id IN ({placeholders})"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&now];
    for id in event_ids {
        params.push(id);
    }
    conn.execute(&sql, &params[..])?;
    Ok(())
}

pub fn mark_failed(db: &Database, event_ids: &[String], err: &str) -> Result<()> {
    if event_ids.is_empty() {
        return Ok(());
    }
    let conn = db.lock();
    let placeholders = std::iter::repeat("?").take(event_ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE local_events
         SET sync_status='FAILED', retry_count = retry_count + 1, last_error = ?1
         WHERE event_id IN ({placeholders})"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&err];
    for id in event_ids {
        params.push(id);
    }
    conn.execute(&sql, &params[..])?;
    Ok(())
}
