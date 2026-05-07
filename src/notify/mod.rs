//! Windows 토스트 / macOS 알림센터 — 우측 하단 소명 팝업 (기획서 §15, §19).
//!
//! `notify-rust` 가 OS 별 백엔드를 자동 선택한다. 클릭/버튼 콜백은 OS 마다
//! 동작이 달라서 1차 MVP 에서는 단순 메시지 팝업만 띄우고, 사용자가 트레이
//! 또는 메인 창의 "근무시간 소명" 메뉴로 진입하도록 안내한다.

use anyhow::Result;
use notify_rust::Notification;

pub fn show_explanation_request(start_label: &str, end_label: &str, duration_minutes: i64) -> Result<()> {
    let summary = "근무시간 소명";
    let body = format!(
        "{start_label} ~ {end_label} 자리비움 {duration_minutes}분이 감지되었습니다.\n\
         소명하지 않은 시간은 휴게시간으로 처리될 수 있습니다."
    );
    Notification::new()
        .summary(summary)
        .body(&body)
        .appname("핀플 PC")
        .show()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("토스트 표시 실패: {e}"))
}
