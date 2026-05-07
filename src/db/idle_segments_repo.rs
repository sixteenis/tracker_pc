//! 자리비움/잠금/앱종료/PC종료/기록없음 구간 저장소.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::params;
use uuid::Uuid;

use super::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    PcIdle,
    PcLocked,
    PcAppClosed,
    PcShutdown,
    NoPcRecord,
}

impl SegmentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SegmentType::PcIdle => "PC_IDLE",
            SegmentType::PcLocked => "PC_LOCKED",
            SegmentType::PcAppClosed => "PC_APP_CLOSED",
            SegmentType::PcShutdown => "PC_SHUTDOWN",
            SegmentType::NoPcRecord => "NO_PC_RECORD",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "PC_IDLE" => Self::PcIdle,
            "PC_LOCKED" => Self::PcLocked,
            "PC_APP_CLOSED" => Self::PcAppClosed,
            "PC_SHUTDOWN" => Self::PcShutdown,
            "NO_PC_RECORD" => Self::NoPcRecord,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplanationStatus {
    Pending,
    Submitted,
    Expired,
    Exempted,
}

impl ExplanationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Submitted => "SUBMITTED",
            Self::Expired => "EXPIRED",
            Self::Exempted => "EXEMPTED",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "SUBMITTED" => Self::Submitted,
            "EXPIRED" => Self::Expired,
            "EXEMPTED" => Self::Exempted,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdleSegment {
    pub id: i64,
    pub segment_id: String,
    pub company_id: String,
    pub employee_id: String,
    pub device_id: String,
    pub work_date: NaiveDate,
    pub segment_type: SegmentType,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub applied_idle_threshold_seconds: i64,
    pub policy_scope: String,
    pub explanation_required: bool,
    pub explanation_deadline: Option<DateTime<Utc>>,
    pub explanation_status: ExplanationStatus,
}

#[derive(Debug, Clone)]
pub struct NewSegment {
    pub company_id: String,
    pub employee_id: String,
    pub device_id: String,
    pub work_date: NaiveDate,
    pub segment_type: SegmentType,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub applied_idle_threshold_seconds: i64,
    pub policy_scope: String,
    pub explanation_deadline: Option<DateTime<Utc>>,
}

pub fn insert(db: &Database, seg: &NewSegment) -> Result<String> {
    let segment_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let duration = seg
        .end_time
        .map(|e| (e - seg.start_time).num_seconds().max(0));
    let conn = db.lock();
    conn.execute(
        "INSERT INTO idle_segments(
            segment_id, company_id, employee_id, device_id, work_date, segment_type,
            start_time, end_time, duration_seconds, applied_idle_threshold_seconds,
            policy_scope, explanation_required, explanation_deadline, explanation_status,
            worktime_reflection_status, created_at, updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,1,?12,'PENDING',NULL,?13,?13)",
        params![
            segment_id,
            seg.company_id,
            seg.employee_id,
            seg.device_id,
            seg.work_date.format("%Y-%m-%d").to_string(),
            seg.segment_type.as_str(),
            seg.start_time.to_rfc3339(),
            seg.end_time.map(|e| e.to_rfc3339()),
            duration,
            seg.applied_idle_threshold_seconds,
            seg.policy_scope,
            seg.explanation_deadline.map(|d| d.to_rfc3339()),
            now,
        ],
    )?;
    Ok(segment_id)
}

pub fn close(db: &Database, segment_id: &str, end_time: DateTime<Utc>) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "UPDATE idle_segments
         SET end_time = ?1,
             duration_seconds = CAST((julianday(?1) - julianday(start_time)) * 86400 AS INTEGER),
             updated_at = ?2
         WHERE segment_id = ?3 AND end_time IS NULL",
        params![end_time.to_rfc3339(), Utc::now().to_rfc3339(), segment_id],
    )?;
    Ok(())
}

pub fn list_pending_for_employee(db: &Database, employee_id: &str) -> Result<Vec<IdleSegment>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(
        "SELECT id, segment_id, company_id, employee_id, device_id, work_date, segment_type,
                start_time, end_time, duration_seconds, applied_idle_threshold_seconds,
                policy_scope, explanation_required, explanation_deadline, explanation_status
         FROM idle_segments
         WHERE employee_id = ?1 AND explanation_status IN ('PENDING','EXPIRED')
         ORDER BY start_time DESC
         LIMIT 200",
    )?;
    let rows = stmt
        .query_map([employee_id], map_segment)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_for_date(db: &Database, employee_id: &str, date: NaiveDate) -> Result<Vec<IdleSegment>> {
    let conn = db.lock();
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, segment_id, company_id, employee_id, device_id, work_date, segment_type,
                start_time, end_time, duration_seconds, applied_idle_threshold_seconds,
                policy_scope, explanation_required, explanation_deadline, explanation_status
         FROM idle_segments
         WHERE employee_id = ?1 AND work_date = ?2
         ORDER BY start_time ASC",
    )?;
    let rows = stmt
        .query_map([employee_id, &date_str], map_segment)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn mark_submitted(db: &Database, segment_id: &str) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "UPDATE idle_segments SET explanation_status='SUBMITTED', updated_at=?1
         WHERE segment_id = ?2",
        params![Utc::now().to_rfc3339(), segment_id],
    )?;
    Ok(())
}

fn map_segment(row: &rusqlite::Row) -> rusqlite::Result<IdleSegment> {
    let work_date_str: String = row.get(5)?;
    let work_date = NaiveDate::parse_from_str(&work_date_str, "%Y-%m-%d")
        .unwrap_or_else(|_| Utc::now().date_naive());
    let start_str: String = row.get(7)?;
    let start_time = DateTime::parse_from_rfc3339(&start_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let end_time = row
        .get::<_, Option<String>>(8)?
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
    let deadline = row
        .get::<_, Option<String>>(13)?
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
    Ok(IdleSegment {
        id: row.get(0)?,
        segment_id: row.get(1)?,
        company_id: row.get(2)?,
        employee_id: row.get(3)?,
        device_id: row.get(4)?,
        work_date,
        segment_type: SegmentType::parse(&row.get::<_, String>(6)?).unwrap_or(SegmentType::PcIdle),
        start_time,
        end_time,
        duration_seconds: row.get(9)?,
        applied_idle_threshold_seconds: row.get(10)?,
        policy_scope: row.get(11)?,
        explanation_required: row.get::<_, i32>(12)? != 0,
        explanation_deadline: deadline,
        explanation_status: ExplanationStatus::parse(&row.get::<_, String>(14)?),
    })
}
