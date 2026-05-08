//! 팀 정보 repository — 현재 mock.

use crate::domain::model::team::Team;

pub fn mock_team() -> Team {
    Team {
        id: 9869,
        name: "개발 (Mock)".to_string(),
    }
}
