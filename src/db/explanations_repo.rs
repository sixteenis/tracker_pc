//! ============================================================================
//! db::explanations_repo — 근무시간 소명 입력 저장소 (`explanations` 테이블).
//! ============================================================================
//!
//! 흐름: 사용자가 입력 화면에서 제출 → `insert()` 로 PENDING 저장
//!       → `auth::Session` 의 access_token 으로 서버 POST
//!       → 성공 시 `mark_synced()` 로 SUCCESS
//!       → 실패해도 로컬에는 남아있어 다음 기회에 재전송 가능.
//!
//! TODO(2차): 재전송 루프 — 현재는 입력 화면에서 1회만 전송 시도. 실패한 PENDING
//! 레코드를 주기적으로 다시 보내는 워커가 없음. `event_sync` 와 같은 1분 주기
//! task 에 통합하거나 `local_events` 와 합쳐 단일 큐로 단순화.
//! TODO(2차): 한 segment 에 대해 사용자가 소명을 여러 번 제출(수정) 한다면
//! 현재는 다른 row 가 추가될 뿐 — 서버측에서 중복 처리 정책 합의 필요.

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

/// 새 소명 입력 저장 + auto-increment row id 반환 (서버 동기화 후 mark_synced 에 사용).
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

/// 서버 전송 성공 → sync_status = SUCCESS.
/// TODO(2차): `synced_at` 컬럼 추가해서 동기화 시각도 기록.
pub fn mark_synced(db: &Database, id: i64) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "UPDATE explanations SET sync_status='SUCCESS' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
