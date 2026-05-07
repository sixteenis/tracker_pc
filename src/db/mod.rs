//! 로컬 SQLite 저장소.
//!
//! - 단일 connection 을 `Mutex` 로 감싸 다중 스레드 공유.
//! - 기획서 §21, §24 의 5개 테이블만 다룸 (auth / local_events / idle_segments / explanations / settings).
//! - 모든 시간은 UTC ISO 8601 (`chrono::DateTime<Utc>` → RFC3339).

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
use rusqlite::Connection;

pub mod auth_repo;
pub mod events_repo;
pub mod explanations_repo;
pub mod idle_segments_repo;
pub mod settings_repo;

const MIGRATION_0001: &str = include_str!("../../migrations/0001_init.sql");

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
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)
            .with_context(|| format!("SQLite open 실패: {}", path.display()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        // poison 발생 시에도 데이터 확보를 위해 그대로 진행.
        match self.conn.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute_batch(MIGRATION_0001).context("마이그레이션 실패")?;
        Ok(())
    }
}
