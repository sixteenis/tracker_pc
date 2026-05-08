//! 출근 상태 repository — 현재 mock.

use chrono::Utc;

use crate::domain::model::attendance::{Attendance, AttendanceStatus};

pub fn mock_attendance() -> Attendance {
    Attendance {
        status: AttendanceStatus::Working,
        work_start_at: Some(Utc::now()),
        work_end_at: None,
    }
}
