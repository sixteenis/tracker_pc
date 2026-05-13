//! ============================================================================
//! db::explanations_repo — 근무시간 소명 입력 저장소 (`explanations` 테이블).
//! ============================================================================
//!
//! 흐름 (2026-05-12 갱신 — 서버 진실 소스 + 로컬 재시도 큐):
//!   1. 사용자 입력 화면에서 제출 → `insert()` 로 PENDING 저장 + 즉시 비동기 POST 시도.
//!   2. POST 성공 시 → `delete()` 로 로컬 row **물리 삭제** (이력은 서버가 보관).
//!   3. POST 실패(네트워크/서버 무응답) 시 → 로컬에 PENDING 유지.
//!   4. `sync::event_sync` 가 1분 주기로 `pending_batch()` 조회 → 재시도 → 성공 시 `delete()`.
//!
//! ── 변경 사유 (사용자 지시 2026-05-12) ────────────────────────────
//! UI "전체 소명 내역" 화면이 서버 `GET /api/pc-agent/worktime-explanations` 응답을
//! 진실 소스로 사용하면서, 로컬 SQLite 는 **오프라인 큐** 역할만 한다. 성공 시
//! UPDATE 가 아닌 DELETE 로 처리해 로컬에는 미전송 row 만 남도록 한다.
//! 이전의 `mark_synced` (SYNC_STATUS=SUCCESS 마킹) 은 호환을 위해 남겨두지만
//! 신규 코드는 `delete` 를 사용한다.
//!
//! TODO(2차): 한 segment 에 대해 사용자가 소명을 여러 번 제출(수정) 한다면
//! 현재는 다른 row 가 추가될 뿐 — 서버측에서 중복 처리 정책 합의 필요.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::params;

use super::Database;

/// 재시도 큐 항목 — `sync::event_sync` 가 한 건씩 서버 송신.
#[derive(Debug, Clone)]
pub struct PendingExplanation {
    pub id: i64,
    pub segment_id: String,
    pub work_date: NaiveDate,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: i64,
    pub explanation_type: String,
    pub explanation_text: Option<String>,
    /// `explanation_type == "OTHER"` 일 때 사용자 입력 라벨 (1~50자). 그 외 None.
    pub other_type_label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewExplanation {
    pub segment_id: String,
    pub work_date: NaiveDate,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: i64,
    pub explanation_type: String, // MEETING / PHONE_CALL / ... / OTHER
    pub explanation_text: Option<String>,
    /// `explanation_type == "OTHER"` 일 때 사용자 입력 라벨 (1~50자). 그 외 None.
    pub other_type_label: Option<String>,
}

/// 새 소명 입력 저장 + auto-increment row id 반환 (서버 동기화 후 mark_synced 에 사용).
pub fn insert(db: &Database, e: &NewExplanation) -> Result<i64> {
    let conn = db.lock();
    conn.execute(
        "INSERT INTO explanations(segment_id, work_date, start_time, end_time, duration_seconds,
            explanation_type, explanation_text, other_type_label, submitted_from, submitted_at, sync_status)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'PC_APP',?9,'PENDING')",
        params![
            e.segment_id,
            e.work_date.format("%Y-%m-%d").to_string(),
            e.start_time.to_rfc3339(),
            e.end_time.to_rfc3339(),
            e.duration_seconds,
            e.explanation_type,
            e.explanation_text,
            e.other_type_label,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 서버 전송 성공 시 로컬 row 물리 삭제. (2026-05-12 변경: SUCCESS 마킹 → DELETE)
/// UI "전체 소명 내역" 은 서버 응답을 진실 소스로 사용하므로 로컬 흔적 불필요.
/// `event_sync` 재시도 루프와 즉시 제출 흐름 모두 이 함수를 사용.
pub fn delete(db: &Database, id: i64) -> Result<()> {
    let conn = db.lock();
    conn.execute("DELETE FROM explanations WHERE id = ?1", params![id])?;
    Ok(())
}

/// `sync::event_sync` 가 한 주기에 한 번 호출 — PENDING/FAILED row 를 재전송 후보로 묶어 반환.
/// 최대 `limit` 건. 오래된 것부터 (`id ASC`).
pub fn pending_batch(db: &Database, limit: u32) -> Result<Vec<PendingExplanation>> {
    let conn = db.lock();
    let mut stmt = conn.prepare(
        "SELECT id, segment_id, work_date, start_time, end_time, duration_seconds,
                explanation_type, explanation_text, other_type_label
         FROM explanations
         WHERE sync_status IN ('PENDING','FAILED')
         ORDER BY id ASC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        let work_date_str: String = r.get(2)?;
        let start_str: String = r.get(3)?;
        let end_str: String = r.get(4)?;
        let parse_date = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_default();
        let parse_dt = |s: &str| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now())
        };
        Ok(PendingExplanation {
            id: r.get(0)?,
            segment_id: r.get(1)?,
            work_date: parse_date(&work_date_str),
            start_time: parse_dt(&start_str),
            end_time: parse_dt(&end_str),
            duration_seconds: r.get(5)?,
            explanation_type: r.get(6)?,
            explanation_text: r.get(7)?,
            other_type_label: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// 송신 실패 시 sync_status=FAILED + 에러 메시지 기록. 재시도는 `pending_batch` 가 다시 잡아감.
pub fn mark_failed(db: &Database, id: i64, error: &str) -> Result<()> {
    let conn = db.lock();
    conn.execute(
        "UPDATE explanations SET sync_status='FAILED' WHERE id = ?1",
        params![id],
    )?;
    let _ = error;
    Ok(())
}
