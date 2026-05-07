//! ============================================================================
//! db::events_repo — 서버 전송 대기 이벤트 큐 (`local_events` 테이블).
//! ============================================================================
//!
//! 흐름:
//!   감지 모듈 → enqueue() → PENDING
//!   sync::event_sync (1분 주기) → mark_success() (SUCCESS) 또는 mark_failed() (FAILED)
//!   FAILED 도 다음 배치에서 다시 시도됨 (`pending_batch` 가 PENDING ∪ FAILED 반환).
//!
//! 멱등성: 모든 이벤트에 UUID `event_id` 부여. 서버는 같은 id 가 두 번 와도
//! 한 번만 저장한다 (기획서 §17, §22).
//!
//! TODO(2차): retry_count 가 일정 임계 (예: 20) 이상이면 DLQ(dead letter) 상태로
//! 분리해서 무한 재시도 막기. 1차 MVP 는 무한 재시도.
//! TODO(2차): 오래된 SUCCESS 이벤트 정기 vacuum (현재 영구 보관 — 디스크 용량 누적).

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

/// 새 이벤트를 큐에 등록하고 발급된 `event_id` 를 반환.
/// 동시에 여러 호출이 와도 UUID 라 충돌 위험은 사실상 없음.
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

/// 다음 배치 전송 대상 (PENDING + FAILED) 을 `limit` 만큼 반환.
/// 오래된 것부터 (id ASC).
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

/// 서버가 "받았다" 로 응답한 event_id 들을 SUCCESS 로 마크.
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

/// 전송 실패한 event_id 들에 retry_count 증가 + 마지막 에러 메시지 기록.
/// 다음 `pending_batch` 가 다시 가져간다.
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
