//! ============================================================================
//! db::idle_segments_repo — 자리비움 구간 저장소 (`idle_segments` 테이블).
//! ============================================================================
//!
//! `segment_type` 5종:
//!   - PC_IDLE       : 키보드/마우스 미입력 (idle_detector 가 생성)
//!   - PC_LOCKED     : Windows 잠금 (1차 MVP 미구현 — session_events stub)
//!   - PC_APP_CLOSED : 앱 정상 종료 후 재기동 사이 (lifecycle 가 NO_PC_RECORD 와 함께)
//!   - PC_SHUTDOWN   : PC 종료 감지 (1차 MVP 미구현)
//!   - NO_PC_RECORD  : 출근 상태인데 heartbeat 끊김 (lifecycle::record_started)
//!
//! 각 segment 는 `applied_idle_threshold_seconds` + `policy_scope` 도 함께 저장.
//! 나중에 관리자가 정책을 바꿔도 과거 segment 가 어떤 기준으로 만들어졌는지 추적 가능.
//!
//! TODO(2차): PC_LOCKED 실제 감지 — `WTSRegisterSessionNotification` + 메시지 윈도우.
//! TODO(2차): PC_SHUTDOWN 감지 — `SetConsoleCtrlHandler` 또는 WM_QUERYENDSESSION
//!            훅으로 종료 직전 ack 이벤트 enqueue.
//! TODO(2차): lunch::classify 결과를 segment 에 저장하는 컬럼 추가
//!            (점심 후보 vs 일반 자리비움). 현재는 모두 동일 처리.
//! TODO(2차): 서버에서 받아온 segment 와 로컬 segment 병합 로직
//!            (`api::list_explanations` 응답을 `idle_segments` 에 upsert).

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

/// 새 segment 삽입 + 새 UUID `segment_id` 반환.
/// `end_time = None` 이면 진행 중 (open) 으로 저장됨 — 나중에 `close()` 호출.
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

/// 진행 중(`end_time = NULL`) segment 의 종료 시각 + duration 을 채워 닫는다.
/// 이미 닫힌 segment 는 무시 (WHERE end_time IS NULL).
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

/// 근로자의 PENDING + EXPIRED 자리비움 구간 (소명 화면용). 최신순, 최대 200건.
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

/// 근로자의 모든 자리비움 구간 (status 무관). 최신순, 최대 500건.
/// 소명 내역 화면에서 탭 필터(전체/소명필요/검토중/승인완료) 용도.
pub fn list_all_for_employee(db: &Database, employee_id: &str) -> Result<Vec<IdleSegment>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(
        "SELECT id, segment_id, company_id, employee_id, device_id, work_date, segment_type,
                start_time, end_time, duration_seconds, applied_idle_threshold_seconds,
                policy_scope, explanation_required, explanation_deadline, explanation_status
         FROM idle_segments
         WHERE employee_id = ?1
         ORDER BY start_time DESC
         LIMIT 500",
    )?;
    let rows = stmt
        .query_map([employee_id], map_segment)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 특정 날짜의 모든 segment (status 무관). 상태 화면 타임라인 그릴 때 사용.
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

/// 사용자가 소명 제출 완료한 segment 의 상태를 SUBMITTED 로 갱신.
/// UI 목록에서 자동으로 사라지게 됨.
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
