//! 근무시간 소명 입력 저장소.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::params;

use super::Database;

#[derive(Debug, Clone)]
pub struct NewExplanation {
    pub segment_id: String,
    pub work_date: NaiveDate,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: i64,
    pub explanation_type: String, // MEETING / PHONE_CALL / ...
    pub explanation_text: Option<String>,
}

pub fn insert(db: &Database, e: &NewExplanation) -> Result<i64> {
    let conn = db.lock();
    conn.execute(
        "INSERT INTO explanations(segment_id, work_date, start_time, end_time, duration_seconds,
            explanation_type, explanation_text, submitted_from, submitted_at, sync_status)
         VALUES(?1,?2,?3,?4,?5,?6,?7,'PC_APP',?8,'PENDING')",
        params![
            e.segment_id,
            e.work_date.format("%Y-%m-%d").to_string(),
            e.start_time.to_rfc3339(),
            e.end_time.to_rfc3339(),
            e.duration_seconds,
            e.explanation_type,
            e.explanation_text,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn mark_synced(db: &Database, id: i64) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "UPDATE explanations SET sync_status='SUCCESS' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
