//! 메인 정보 도메인 서비스 — `get_main2.jsp` 결과 캐싱.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::domain::model::main_info::MainInfo;

static CURRENT: Lazy<RwLock<Option<MainInfo>>> = Lazy::new(|| RwLock::new(None));

pub fn current() -> Option<MainInfo> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

pub fn set(info: MainInfo) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(info);
    }
}

pub fn clear() {
    if let Ok(mut g) = CURRENT.write() {
        *g = None;
    }
}
