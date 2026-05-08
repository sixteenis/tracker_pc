//! 출근 상태 도메인 모델.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttendanceStatus {
    Working,
    BeforeWork,
    AfterWork,
    Outing,
    Leave,
    BusinessTrip,
    Unknown,
}

impl Default for AttendanceStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl AttendanceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Working => "출근 중",
            Self::BeforeWork => "출근 전",
            Self::AfterWork => "퇴근 후",
            Self::Outing => "외출 중",
            Self::Leave => "연차",
            Self::BusinessTrip => "출장",
            Self::Unknown => "알 수 없음",
        }
    }
    pub fn enables_tracking(&self) -> bool {
        matches!(self, Self::Working)
    }
}

#[derive(Debug, Clone)]
pub struct Attendance {
    pub status: AttendanceStatus,
    pub work_start_at: Option<DateTime<Utc>>,
    pub work_end_at: Option<DateTime<Utc>>,
}
