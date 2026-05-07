//! ============================================================================
//! notify — OS 토스트 알림 (기획서 §15, §19).
//! ============================================================================
//!
//! `notify-rust` 가 OS 별 백엔드를 자동 선택:
//!   - Windows : WinRT Toast Notifications
//!   - macOS   : NSUserNotificationCenter
//!   - Linux   : org.freedesktop.Notifications (D-Bus)
//!
//! TODO(2차 핵심): 토스트의 "소명하기" 버튼 클릭 → 메인 창 전면 + 해당 segment_id
//! 입력 화면 진입. WinRT 의 ToastActivator + COM IPC 또는 별도 IPC 채널 필요.
//! 현재는 단순 메시지 팝업만 띄우고, 사용자가 트레이/메인 창 "근무시간 소명"
//! 메뉴로 직접 진입하도록 유도.
//! TODO(2차): macOS 의 NSUserNotification 은 deprecated — UNUserNotificationCenter
//! 로 마이그레이션 필요. notify-rust 가 자동 처리하지만 콜백 활용 시 직접 FFI.

use anyhow::Result;
use notify_rust::Notification;

/// 백그라운드 OS 스레드에서 토스트를 띄운다 — UI 스레드 절대 블로킹 안 함.
///
/// notify-rust 가 일부 환경(특히 unbundled macOS) 에서 LaunchServices 다이얼로그를
/// 띄우는 부작용을 가지므로, fire-and-forget 으로 별도 thread 에서 실행하고
/// 결과는 무시한다.
pub fn show_explanation_request_async(
    start_label: String,
    end_label: String,
    duration_minutes: i64,
) {
    std::thread::spawn(move || {
        if let Err(e) =
            show_explanation_request(&start_label, &end_label, duration_minutes)
        {
            tracing::debug!(error = %e, "토스트 표시 실패 (무시)");
        }
    });
}

/// 동기 호출 — 직접 호출 시 UI 스레드를 블로킹할 수 있으므로 가능하면
/// `show_explanation_request_async` 를 사용하라.
/// 자동 사라짐 시간은 OS 기본값 (Windows ~5초, macOS ~15초).
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
