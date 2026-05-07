// ============================================================================
// ui::tray — 시스템 트레이/메뉴바 아이콘 + 백그라운드 실행 지원.
// ============================================================================
//
// ── 구조 (이슈 #1, #2 대응) ────────────────────────────────────────────
// `tray-icon` 의 `MenuEvent::receiver()` / `TrayIconEvent::receiver()` 는 전역
// crossbeam 채널이라 어느 thread 에서든 polling 가능. 본 모듈은 **별도 OS
// thread** 를 띄워 거기서 polling 한다 — 메인 윈도우가 숨겨져 있어 eframe
// `update()` 가 호출되지 않더라도 트레이 메뉴 클릭이 즉시 잡힌다. polling
// thread 는 이벤트를 mpsc 로 메인 UI 에 전달하면서 동시에 `ctx.request_repaint()`
// 로 `update()` 를 강제 깨운다.
//
// ── Quit 안전장치 ──────────────────────────────────────────────────────
// 사용자가 "종료" 를 누르면 polling thread 자체가 2초 뒤 `process::exit(0)` 을
// 예약한다. 정상 경로(UI cleanup → run_native 반환)가 빠르면 그 전에 프로세스가
// 죽어 이 안전장치 thread 도 함께 사라진다. 만약 UI 가 어떤 이유로 멈춰 있으면
// 강제로 프로세스를 종료시켜 사용자가 "종료가 안 됨" 을 겪지 않게 한다.
//
// 플랫폼 별 동작:
//   - Windows : 작업 표시줄 우측 알림 영역 아이콘 (전형적인 트레이)
//   - macOS   : 상단 메뉴바 아이콘
//   - Linux   : 데스크톱 환경에 따라 다름 (StatusNotifier / AppIndicator)

use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use eframe::egui;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// 트레이 메뉴 클릭 결과. `update()` 가 라우팅에 반영.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// 창 보이기 + 포커스 + 최소화 해제
    Show,
    /// Show + 라우트를 ExplanationList 로
    OpenExplanation,
    /// 정말로 종료
    Quit,
}

/// 트레이 핸들. 드롭되면 트레이 아이콘이 사라지므로 `PinpleApp` 이 보관.
pub struct TrayHandle {
    _tray: TrayIcon,
    rx: mpsc::Receiver<TrayCommand>,
}

impl TrayHandle {
    /// 트레이 + 메뉴 + 아이콘 생성 + 폴링 thread 시작.
    /// macOS 는 메인 스레드에서 호출되어야 함 (eframe `App::new` 가 메인 스레드).
    ///
    /// `ctx` 는 메인 윈도우의 egui Context — polling thread 가 이벤트를 받을 때마다
    /// `ctx.request_repaint()` 로 UI 를 깨운다.
    pub fn new(ctx: egui::Context) -> Result<Self> {
        let menu = Menu::new();
        let show_item = MenuItem::new("화면 열기", true, None);
        let exp_item = MenuItem::new("근무시간 소명", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::new("종료", true, None);

        let show_id = show_item.id().clone();
        let explanation_id = exp_item.id().clone();
        let quit_id = quit_item.id().clone();

        menu.append(&show_item)?;
        menu.append(&exp_item)?;
        menu.append(&separator)?;
        menu.append(&quit_item)?;

        let icon = make_default_icon();

        let tray = TrayIconBuilder::new()
            .with_tooltip("핀플 PC")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()?;

        // ── 폴링 thread ──────────────────────────────────────────────
        // 메인 UI 가 숨겨져 update() 가 안 도는 동안에도 이 thread 는 계속 실행되며
        // 이벤트를 수신, mpsc 채널로 UI 에 전달하고 request_repaint 로 깨운다.
        let (tx, rx) = mpsc::channel();
        spawn_poll_thread(ctx, tx, show_id, explanation_id, quit_id);

        Ok(Self { _tray: tray, rx })
    }

    /// UI 의 update() 가 호출 — 큐에 쌓인 명령을 한 건 꺼낸다.
    pub fn poll(&self) -> Option<TrayCommand> {
        self.rx.try_recv().ok()
    }
}

fn spawn_poll_thread(
    ctx: egui::Context,
    tx: mpsc::Sender<TrayCommand>,
    show_id: MenuId,
    explanation_id: MenuId,
    quit_id: MenuId,
) {
    std::thread::Builder::new()
        .name("tray-poller".into())
        .spawn(move || {
            let menu_rx = MenuEvent::receiver();
            let tray_rx = TrayIconEvent::receiver();
            loop {
                let mut woke = false;

                // 메뉴 클릭 처리
                while let Ok(ev) = menu_rx.try_recv() {
                    let cmd = if ev.id == show_id {
                        Some(TrayCommand::Show)
                    } else if ev.id == explanation_id {
                        Some(TrayCommand::OpenExplanation)
                    } else if ev.id == quit_id {
                        // 사용자가 명시적 종료 요청 — 안전장치 활성화.
                        // UI 정상 경로가 1.5~2초 안에 끝나면 process 가 그 전에 사라져
                        // 본 thread 도 함께 죽음. 못 끝내면 강제 exit.
                        std::thread::Builder::new()
                            .name("tray-quit-fallback".into())
                            .spawn(|| {
                                std::thread::sleep(Duration::from_millis(2000));
                                tracing::warn!("정상 종료가 2초 내 완료되지 않아 강제 종료합니다");
                                std::process::exit(0);
                            })
                            .ok();
                        Some(TrayCommand::Quit)
                    } else {
                        None
                    };
                    if let Some(c) = cmd {
                        let _ = tx.send(c);
                        woke = true;
                    }
                }

                // 트레이 아이콘 좌클릭/더블클릭 → 창 보이기 (Windows 일관성)
                while tray_rx.try_recv().is_ok() {
                    let _ = tx.send(TrayCommand::Show);
                    woke = true;
                }

                if woke {
                    ctx.request_repaint();
                }

                // 50ms 폴링 — 사용자 체감 즉시 반응 + CPU 부하 무시할만한 수준.
                std::thread::sleep(Duration::from_millis(50));
            }
        })
        .expect("tray-poller thread spawn 실패");
}

/// 32×32 단색 오렌지 원형 아이콘을 즉석에서 생성 (외부 파일 의존 없음).
/// TODO(2차): `resources/icon.ico` 또는 `.png` 로 교체해서 디자인 통일.
fn make_default_icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba: Vec<u8> = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let cx = (SIZE as f32 - 1.0) / 2.0;
    let cy = cx;
    let radius = SIZE as f32 / 2.0 - 1.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if (dx * dx + dy * dy).sqrt() <= radius {
                rgba.extend_from_slice(&[230, 68, 32, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("32x32 RGBA 아이콘 생성 실패")
}
