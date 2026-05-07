//! ============================================================================
//! sync::update_check — 12시간 주기 앱 업데이트 정보 폴링.
//! ============================================================================
//!
//! - `force_update = true` 이면서 현재 버전이 `minimum_required_version` 미만이면
//!   `can_track_time = false` 강제 → 감지/heartbeat 즉시 중지.
//! - 응답을 `update_info` 에 저장 → UI 의 "업데이트" 메뉴/뱃지가 자동 표시.
//!
//! ── 미구현 ─────────────────────────────────────────────────────────────
//! 자동 다운로드/실행은 안 함. UI 의 `update_view` 가 download_url 링크만 제공.
//!
//! TODO(2차): `download_url` 의 .exe (또는 .pkg) 를 임시 디렉토리에 다운로드 →
//!   서명 검증 → 실행 → 자기 자신 종료. Inno Setup 인스톨러는 자동으로 기존 앱
//!   종료/덮어쓰기 처리하므로 다운로드 + 실행만 클라이언트 책임.
//! TODO(서버 연동): 응답에 SHA256 해시 추가해서 다운로드 무결성 검증.

use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::api::types::UpdateCheckRequest;
use crate::app::AppState;

/// 시작 후 15초 대기 → 업데이트 확인 → 12시간 주기 반복.
pub async fn run(state: Arc<AppState>) {
    // 시작 직후 1회 + 이후 12시간 주기.
    let initial_delay = Duration::from_secs(15);
    let main_interval = Duration::from_secs(
        state.config.intervals.update_check_interval_seconds.max(3600),
    );
    tokio::time::sleep(initial_delay).await;

    loop {
        let req = UpdateCheckRequest {
            current_version: state.config.app.app_version.clone(),
            os: if cfg!(windows) { "windows" } else { "macos" }.to_string(),
        };
        match state.api.update_check(req).await {
            Ok(info) => {
                if info.force_update
                    && version_lt(
                        &info.current_version,
                        &info.minimum_required_version,
                    )
                {
                    warn!(latest = %info.latest_version, "강제 업데이트 필요 — 추적 일시 중지");
                    if let Ok(mut s) = state.status.write() {
                        s.can_track_time = false;
                    }
                }
                info!(latest = %info.latest_version, required = info.update_required, "업데이트 확인 완료");
                if let Ok(mut w) = state.update_info.write() {
                    *w = Some(info);
                }
            }
            Err(e) => warn!(error = %e, "업데이트 확인 실패"),
        }
        tokio::time::sleep(main_interval).await;
    }
}

/// "0.1.0" < "0.2.0" 같은 단순 SemVer-ish 비교. pre-release/build metadata 무시.
/// 잘못된 형식이면 0 으로 폴백 (안전 기본값 — 강제 업데이트 트리거 안 함).
fn version_lt(a: &str, b: &str) -> bool {
    fn parse(v: &str) -> Vec<u32> {
        v.split('.').filter_map(|p| p.parse().ok()).collect()
    }
    let pa = parse(a);
    let pb = parse(b);
    let len = pa.len().max(pb.len());
    for i in 0..len {
        let xa = pa.get(i).copied().unwrap_or(0);
        let xb = pb.get(i).copied().unwrap_or(0);
        if xa < xb {
            return true;
        }
        if xa > xb {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::version_lt;
    #[test]
    fn lt_basic() {
        assert!(version_lt("0.1.0", "0.2.0"));
        assert!(version_lt("0.1.0", "1.0.0"));
        assert!(!version_lt("1.0.0", "0.9.9"));
        assert!(!version_lt("1.0.0", "1.0.0"));
    }
}
