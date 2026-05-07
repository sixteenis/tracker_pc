// ============================================================================
// ui::tray — 시스템 트레이/메뉴바 아이콘 + 백그라운드 실행 지원.
// ============================================================================
//
// 사용자가 창의 X(닫기) 버튼을 눌러도 앱은 종료되지 않고 트레이로 들어간다.
// 트레이 메뉴에서 "화면 열기" 또는 "근무시간 소명" 으로 다시 창을 띄우거나
// "종료" 로 정말로 종료할 수 있다.
//
// 동작 동안:
//   - tokio 런타임은 그대로 살아있음 → idle 감지 / heartbeat / 이벤트 배치
//     모두 정상 진행. 자리비움 발생 시 OS 토스트가 뜨고 `idle_segments` 누적.
//
// 플랫폼 별 동작:
//   - Windows : 작업 표시줄 우측 알림 영역 아이콘 (전형적인 트레이)
//   - macOS   : 상단 메뉴바 아이콘
//   - Linux   : 데스크톱 환경에 따라 다름 (StatusNotifier / AppIndicator)
//
// TODO(2차): 트레이 아이콘 우클릭/좌클릭 동작 OS 별 일관화 (현재는 메뉴 클릭만 안전).
// TODO(2차): 자리비움 N건 누적 시 트레이 아이콘에 빨간 뱃지 (Windows: NIM_MODIFY,
//            macOS: NSStatusItem.button.image overlay).

use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// 트레이 메뉴 클릭 결과. `update()` 가 라우팅에 반영.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// 창 보이기 + 포커스
    Show,
    /// 창 보이기 + 포커스 + 라우트를 ExplanationList 로
    OpenExplanation,
    /// 정말로 종료 (record_stopped 후 프로세스 exit)
    Quit,
}

/// 트레이 핸들. 드롭되면 트레이 아이콘이 사라지므로 `PinpleApp` 이 보관.
pub struct TrayHandle {
    _tray: TrayIcon,
    show_id: MenuId,
    explanation_id: MenuId,
    quit_id: MenuId,
}

impl TrayHandle {
    /// 트레이 + 메뉴 + 아이콘 생성. macOS 는 메인 스레드에서 호출되어야 함
    /// (eframe `App::new` 가 메인 스레드라 OK).
    pub fn new() -> Result<Self> {
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

        Ok(Self { _tray: tray, show_id, explanation_id, quit_id })
    }

    /// 매 UI 프레임마다 호출 — 큐에 쌓인 메뉴/클릭 이벤트를 한 건 처리.
    /// 여러 이벤트가 동시에 와도 다음 프레임에 순차 처리됨.
    pub fn poll(&self) -> Option<TrayCommand> {
        // 메뉴 클릭 우선 처리.
        if let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.show_id {
                return Some(TrayCommand::Show);
            }
            if ev.id == self.explanation_id {
                return Some(TrayCommand::OpenExplanation);
            }
            if ev.id == self.quit_id {
                return Some(TrayCommand::Quit);
            }
        }
        // Windows 트레이 아이콘 좌클릭 시 창 띄우기.
        if let Ok(_ev) = TrayIconEvent::receiver().try_recv() {
            return Some(TrayCommand::Show);
        }
        None
    }
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
                // 핀플 오렌지
                rgba.extend_from_slice(&[230, 68, 32, 255]);
            } else {
                // 투명
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("32x32 RGBA 아이콘 생성 실패")
}
