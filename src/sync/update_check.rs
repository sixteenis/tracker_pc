//! 12시간 주기 앱 업데이트 확인. 강제 업데이트 시 PC 기록 기능을 중지한다.

use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::api::types::UpdateCheckRequest;
use crate::app::AppState;

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

/// "0.1.0" / "1.2.3" 같은 SemVer-ish 비교. 잘못된 형식은 false 로.
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
