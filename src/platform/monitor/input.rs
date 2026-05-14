//! ============================================================================
//! monitor::input — 마지막 사용자 입력 이후 경과 시간 (초) 측정.
//! ============================================================================
//!
//! - Windows : `GetLastInputInfo` Win32 API (운영 대상).
//! - macOS   : `ioreg -c IOHIDSystem` 의 `HIDIdleTime` 파싱 (개발/테스트용).
//! - Linux   : stub 0 (또는 `PINPLE_FAKE_IDLE`).
//!
//! 환경변수 `PINPLE_FAKE_IDLE=N` 으로 강제 idle 값 주입 (테스트용).
//!
//! ── 실패 시 fallback (2026-05-14) ─────────────────────────────────────
//! OS 호출이 가끔 실패해도 segment 가 깨지지 않도록 **직전 측정값 + 폴링 간격**
//! 만큼 idle 누적을 이어간다. 실패 시 0 반환하던 이전 동작은 idle == 0 으로 보여
//! idle_detector 가 "사용자 복귀" 로 오해 → segment close 후 다시 OPEN 반복으로
//! 24분 자리비움이 여러 row 로 쪼개지던 사고 원인이었다.
//!
//! macOS 구현은 매 호출마다 짧은 자식 프로세스를 띄우므로 5초 폴링 정도까지만
//! 적합하다. 운영 환경(Windows) 에서는 순수 Win32 호출이라 비용이 거의 없다.
//!
//! TODO(2차): macOS 도 `IOKit` 직접 FFI 로 교체 (자식 프로세스 안 띄우게).
//! TODO(2차): Linux 도 X11 `XScreenSaverQueryInfo` 또는 Wayland idle protocol 지원.

use std::sync::atomic::{AtomicU64, Ordering};

/// 직전 측정값. OS 호출 실패 시 폴링 간격(5초) 만큼 더해서 누적을 이어간다.
static LAST_IDLE: AtomicU64 = AtomicU64::new(0);

/// 폴링 간격 — 호출 실패 시 fallback 누적 단위. idle_check_interval_seconds 와 일치.
const POLL_INTERVAL_SECONDS: u64 = 5;

/// OS 호출 실패 시 호출 — 직전값 + 폴링 간격 반환 후 LAST_IDLE 갱신.
fn fallback_advance() -> u64 {
    let prev = LAST_IDLE.load(Ordering::Relaxed);
    let next = prev.saturating_add(POLL_INTERVAL_SECONDS);
    LAST_IDLE.store(next, Ordering::Relaxed);
    next
}

/// 성공 측정 시 호출 — LAST_IDLE 동기화.
fn record(v: u64) -> u64 {
    LAST_IDLE.store(v, Ordering::Relaxed);
    v
}

/// 환경변수 `PINPLE_FAKE_IDLE=<seconds>` 가 있으면 OS API 대신 그 값을 반환.
/// 모든 OS 의 `idle_seconds()` 가 호출 첫머리에서 한 번 확인한다.
fn fake_override() -> Option<u64> {
    std::env::var("PINPLE_FAKE_IDLE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
}

/// Windows 운영 구현. `GetTickCount` 와의 차이로 ms → s 환산.
/// 시스템 부팅 후 49.7일 후 GetTickCount overflow → saturating_sub 로 음수 방지.
#[cfg(windows)]
pub fn idle_seconds() -> u64 {
    if let Some(v) = fake_override() {
        tracing::debug!(fake = v, "input::idle_seconds [PINPLE_FAKE_IDLE]");
        return record(v);
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
            let secs = (elapsed_ms / 1000) as u64;
            tracing::trace!(secs, "input::idle_seconds [Windows OK]");
            record(secs)
        } else {
            tracing::warn!("input::idle_seconds [Windows FAIL — GetLastInputInfo] — 직전값 + 폴링간격 fallback");
            fallback_advance()
        }
    }
}

/// macOS 테스트용 구현. ioreg 자식 프로세스 spawn 후 stdout 파싱.
/// HIDIdleTime 단위는 나노초 → / 1_000_000_000 으로 초 변환.
#[cfg(target_os = "macos")]
pub fn idle_seconds() -> u64 {
    if let Some(v) = fake_override() {
        tracing::debug!(fake = v, "input::idle_seconds [PINPLE_FAKE_IDLE]");
        return record(v);
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
        _ => {
            tracing::warn!("input::idle_seconds [macOS FAIL — ioreg] — 직전값 + 폴링간격 fallback");
            return fallback_advance();
        }
    };
    let text = match std::str::from_utf8(&out) {
        Ok(t) => t,
        Err(_) => {
            tracing::warn!("input::idle_seconds [macOS FAIL — UTF-8 파싱] — fallback");
            return fallback_advance();
        }
    };
    for line in text.lines() {
        if let Some(idx) = line.find("HIDIdleTime") {
            let rest = &line[idx..];
            if let Some(eq) = rest.find('=') {
                let val = rest[eq + 1..].trim();
                if let Ok(ns) = val.parse::<u64>() {
                    let secs = ns / 1_000_000_000;
                    tracing::trace!(secs, "input::idle_seconds [macOS OK]");
                    return record(secs);
                }
            }
        }
    }
    tracing::warn!("input::idle_seconds [macOS FAIL — HIDIdleTime 라인 없음] — fallback");
    fallback_advance()
}

/// Linux/기타 — 1차 MVP 미구현. 항상 0 반환 (자리비움 영원히 발생 안 함).
/// PINPLE_FAKE_IDLE 환경변수로만 강제 가능.
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn idle_seconds() -> u64 {
    fake_override().map(record).unwrap_or(0)
}
