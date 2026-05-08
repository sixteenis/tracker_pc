//! 출근 상태 도메인 서비스 (현재 mock).

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::data::repository::attendance_repository;
use crate::domain::model::attendance::Attendance;

static CURRENT: Lazy<RwLock<Option<Attendance>>> = Lazy::new(|| RwLock::new(None));

pub fn current() -> Option<Attendance> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

pub fn set(att: Attendance) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(att);
    }
}

pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

/// 현재 mock — 항상 출근 중.
pub fn fetch_mock() -> Attendance {
    attendance_repository::mock_attendance()
}
