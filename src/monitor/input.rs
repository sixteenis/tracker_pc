//! 마지막 사용자 입력 이후 경과 시간 (초).
//!
//! - Windows: `GetLastInputInfo` Win32 API (운영 대상).
//! - macOS:   `ioreg -c IOHIDSystem` 의 `HIDIdleTime` 파싱 (개발/테스트용).
//! - Linux:   stub 0 (또는 PINPLE_FAKE_IDLE).
//!
//! macOS 구현은 매 호출마다 짧은 자식 프로세스를 띄우므로 5 초 폴링 정도까지만
//! 적합하다. 운영 환경(Windows) 에서는 순수 Win32 호출이라 비용이 거의 없다.

/// 환경변수로 idle 값을 강제 주입할 수 있는 공통 훅 (모든 OS).
fn fake_override() -> Option<u64> {
    std::env::var("PINPLE_FAKE_IDLE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
}

#[cfg(windows)]
pub fn idle_seconds() -> u64 {
    if let Some(v) = fake_override() {
        return v;
    }
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii).as_bool() {
            let now = GetTickCount();
            let elapsed_ms = now.saturating_sub(lii.dwTime);
            (elapsed_ms / 1000) as u64
        } else {
            0
        }
    }
}

#[cfg(target_os = "macos")]
pub fn idle_seconds() -> u64 {
    if let Some(v) = fake_override() {
        return v;
    }
    use std::process::Command;

    // ioreg 출력 예시 한 라인:
    //   "HIDIdleTime" = 4123456789
    // 단위는 나노초.
    let out = match Command::new("/usr/sbin/ioreg")
        .args(["-c", "IOHIDSystem"])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return 0,
    };
    let text = match std::str::from_utf8(&out) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    for line in text.lines() {
        if let Some(idx) = line.find("HIDIdleTime") {
            let rest = &line[idx..];
            if let Some(eq) = rest.find('=') {
                let val = rest[eq + 1..].trim();
                if let Ok(ns) = val.parse::<u64>() {
                    return ns / 1_000_000_000;
                }
            }
        }
    }
    0
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn idle_seconds() -> u64 {
    fake_override().unwrap_or(0)
}
