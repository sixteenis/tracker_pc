//! 팀 정보 도메인 서비스 (현재 mock).

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::data::repository::team_repository;
use crate::domain::model::team::Team;

static CURRENT: Lazy<RwLock<Option<Team>>> = Lazy::new(|| RwLock::new(None));

pub fn current() -> Option<Team> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

pub fn set(team: Team) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(team);
    }
}

pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}

pub fn fetch_mock() -> Team {
    team_repository::mock_team()
}
