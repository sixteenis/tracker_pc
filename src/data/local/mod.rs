//! ============================================================================
//! db — 로컬 SQLite 저장소.
//! ============================================================================
//!
//! - 단일 connection 을 `Mutex` 로 감싸 다중 스레드 공유.
//! - 기획서 §21, §24 의 5개 테이블만 다룸 (auth / local_events / idle_segments
//!   / explanations / settings).
//! - 모든 시간은 UTC ISO 8601 (`chrono::DateTime<Utc>` → RFC3339 문자열).
//!
//! ── 마이그레이션 정책 ─────────────────────────────────────────────────────
//! `migrations/0001_init.sql` 단일 파일을 `IF NOT EXISTS` 로 매 실행마다 적용.
//!
//! TODO(2차): 정식 마이그레이션 도구(refinery / sqlx-migrate)로 교체. 0002,
//! 0003... 순차 적용 + `schema_versions` 테이블로 멱등성 보장.
//! TODO(2차): WAL 백업 / 손상 복구. 현재 SQLite 파일이 깨지면 사용자 데이터 디렉토리
//! 의 `pinple.db` 를 수동 삭제 후 재시작하는 방법밖에 없음.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
use rusqlite::Connection;

pub mod auth_repo;
pub mod events_repo;
pub mod explanations_repo;
pub mod idle_segments_repo;
pub mod settings_repo;

const MIGRATION_0001: &str = include_str!("../../../migrations/0001_init.sql");

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// 디스크 파일을 열거나 새로 생성. 부모 디렉토리가 없으면 자동 생성.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)
            .with_context(|| format!("SQLite open 실패: {}", path.display()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// 단위 테스트용 in-memory DB. 프로세스 종료 시 사라짐.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// 모든 repo 가 공유하는 connection 잠금. **`await` 너머로 들고 있으면 안 됨**
    /// — 동기 함수 안에서만 짧게 사용.
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        // poison 발생 시에도 데이터 확보를 위해 그대로 진행.
        match self.conn.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    /// `migrations/0001_init.sql` 적용. 매 실행 시 호출되며 `IF NOT EXISTS` 라
    /// 이미 적용돼 있으면 no-op.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute_batch(MIGRATION_0001).context("마이그레이션 실패")?;
        // 기존 DB 호환: `explanations.other_type_label` 컬럼 추가 (없을 때만).
        // 정식 마이그레이션 도구 도입 전까지 ad-hoc 처리.
        ensure_column(&conn, "explanations", "other_type_label", "TEXT")?;
        Ok(())
    }
}

/// 테이블에 컬럼이 없으면 ALTER 로 추가. 있으면 no-op (멱등).
fn ensure_column(conn: &Connection, table: &str, column: &str, col_type: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|name| name == column);
    if !exists {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}"))?;
    }
    Ok(())
}
