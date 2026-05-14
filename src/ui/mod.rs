// ============================================================================
// ui — egui 기반 메인 UI 라우터.
// ============================================================================
//
// `Route` enum 으로 7개 화면 분기:
//   - Login            : 로그인 화면 (오렌지 카드 디자인)
//   - Status           : 메인 상태 (출근/통계/타임라인/소명 진입)
//   - ExplanationList  : 자리비움 목록 + "소명하기" 버튼
//   - ExplanationInput : 소명 사유/내용 입력
//   - Settings         : 환경설정 (3 탭: 일반/감지/계정)
//   - UpdateNotice     : 업데이트 안내
//   - Disabled         : 요금제 미포함 안내
//
// 공통 컴포넌트:
//   - 색상 팔레트 (ORANGE/NAVY/BG/GRAY_TEXT/GREEN_STATUS/TIMELINE_*)
//   - `orange_header()` 서브페이지 공용 헤더 패널
//   - 한글 폰트 자동 등록 (Windows 맑은고딕 / macOS AppleSDGothicNeo / Linux NotoSansCJK)
//
// TODO(2차): UI repaint 가 1초 주기 폴링. tokio watch 채널을 두고 idle / 동기화
// 이벤트 시점에만 `ctx.request_repaint()` 부르는 게 더 효율적.
// TODO(2차): 다크 모드 지원 — 현재 light visuals 고정.

mod disabled_view;
mod explanation_input_view;
pub mod explanation_list_view;
mod login_view;
pub mod settings_view;
mod status_view;
mod tray;
mod update_view;

use std::sync::Arc;

use eframe::{egui, App, CreationContext, Frame};
use egui::{FontData, FontDefinitions, FontFamily};
use tracing::info;

use crate::app::AppState;

// ── 공통 색상 ──────────────────────────────────────────────────
pub const ORANGE: egui::Color32 = egui::Color32::from_rgb(230, 68, 32);
pub const NAVY: egui::Color32 = egui::Color32::from_rgb(24, 35, 65);
pub const BG: egui::Color32 = egui::Color32::from_rgb(244, 242, 238);
pub const GRAY_TEXT: egui::Color32 = egui::Color32::from_rgb(140, 140, 145);
pub const GREEN_STATUS: egui::Color32 = egui::Color32::from_rgb(46, 200, 110);
pub const TIMELINE_ACTIVE: egui::Color32 = egui::Color32::from_rgb(56, 201, 135);
pub const TIMELINE_IDLE: egui::Color32 = egui::Color32::from_rgb(255, 163, 0);
pub const TIMELINE_LOCKED: egui::Color32 = egui::Color32::from_rgb(160, 160, 165);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Login,
    Status,
    /// 소명 내역 화면. `today_only=true` 면 오늘 발생분만 필터 + 타이틀 "오늘 발생한 소명".
    /// `today_only=false` 는 기존 "전체 소명 내역" + 사용자 필터 탭 유지.
    ExplanationList { today_only: bool },
    ExplanationInput { segment_id: String },
    Settings,
    UpdateNotice,
    Disabled,
}

pub struct PinpleApp {
    state: Arc<AppState>,
    route: Route,
    login_form: login_view::LoginForm,
    explanation_form: explanation_input_view::ExplanationForm,
    last_toast_at: Option<chrono::DateTime<chrono::Utc>>,
    settings_tab: settings_view::SettingsTab,
    settings_ui: settings_view::SettingsUi,
    explanation_filter: explanation_list_view::ExplanationFilter,
    /// 트레이 아이콘 핸들. 드롭되면 트레이에서 사라지므로 보관.
    /// 초기화 실패 시 `None` — 그래도 앱은 정상 동작 (창 닫으면 그냥 종료).
    tray: Option<tray::TrayHandle>,
    /// 트레이 메뉴 "종료" 또는 명시적 종료 시 true. 일반 X 버튼 클릭은 false 유지.
    really_quit: bool,
}

impl PinpleApp {
    pub fn new(cc: &CreationContext<'_>, state: Arc<AppState>) -> Self {
        install_korean_font(&cc.egui_ctx);
        setup_visuals(&cc.egui_ctx);

        let login_form = login_view::LoginForm::default();
        let initial_route = if state.is_logged_in() {
            Route::Status
        } else {
            kick_auto_login(state.clone());
            Route::Login
        };

        // 트레이 폴링 thread 가 ctx.request_repaint() 로 update() 를 깨워야 하므로
        // egui Context 를 복제해서 전달.
        let tray = match tray::TrayHandle::new(cc.egui_ctx.clone()) {
            Ok(t) => {
                info!("트레이 아이콘 초기화 완료 — 창 닫아도 백그라운드에서 계속 동작합니다");
                Some(t)
            }
            Err(e) => {
                tracing::warn!(error = %e, "트레이 아이콘 초기화 실패 — 창 닫으면 앱이 종료됩니다");
                None
            }
        };

        Self {
            state,
            route: initial_route,
            login_form,
            explanation_form: explanation_input_view::ExplanationForm::default(),
            last_toast_at: None,
            settings_tab: settings_view::SettingsTab::General,
            settings_ui: settings_view::SettingsUi::default(),
            explanation_filter: explanation_list_view::ExplanationFilter::default(),
            tray,
            really_quit: false,
        }
    }
}

impl App for PinpleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // ── 트레이 메뉴 이벤트 처리 ───────────────────────────────────────
        if let Some(tray) = &self.tray {
            if let Some(cmd) = tray.poll() {
                use tray::TrayCommand::*;
                match cmd {
                    Show => {
                        // 숨김 + 최소화 양쪽을 모두 풀고 포커스 가져오기.
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    OpenExplanation => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        if self.state.is_logged_in() {
                            // 트레이 "근무시간 소명" 진입은 전체 내역 화면으로.
                            self.route = Route::ExplanationList { today_only: false };
                        }
                    }
                    Quit => {
                        self.really_quit = true;
                        // APP_STOPPED 이벤트 enqueue (다음 배치에서 전송).
                        let _ = crate::platform::monitor::lifecycle::record_stopped(&self.state);

                        // 트레이 아이콘 명시적 드롭 — TrayIcon::Drop 이 NIM_DELETE 호출하며
                        // 트레이에서 즉시 제거 (Windows ghost icon 방지).
                        self.tray = None;

                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);

                        // force-exit fallback 은 tray polling thread 안에 있음.
                        // 여기서 별도로 안 띄움.
                    }
                }
            }
        }

        // ── X(닫기) 클릭 가로채기 — 종료 대신 창 숨기기 ───────────────────
        // really_quit = true 인 경우는 트레이 "종료" 가 이미 처리했으므로 그대로 닫힘.
        // 트레이가 없으면 가로채지 않음 (사용자가 창 다시 띄울 방법이 없으므로).
        if self.tray.is_some()
            && !self.really_quit
            && ctx.input(|i| i.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Minimized(true) 는 Visible(false) 와 달리 Windows 메시지 루프를 유지.
            // SW_HIDE 로 숨기면 update() 가 불리지 않아 트레이 명령이 처리되지 않음.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            tracing::info!("창을 최소화 — 백그라운드에서 idle 감지가 계속됩니다");
        }

        if matches!(self.route, Route::Login) && self.state.is_logged_in() {
            // PIN+ 미사용도 메인 화면 진입 — 헤더 배지에서 안내한다.
            self.route = Route::Status;
        }

        match self.route.clone() {
            Route::Status => {
                status_view::show(ctx, &self.state, &mut self.route);
            }
            Route::ExplanationList { today_only } => {
                explanation_list_view::show(
                    ctx,
                    &self.state,
                    &mut self.route,
                    &mut self.last_toast_at,
                    &mut self.explanation_filter,
                    today_only,
                );
            }
            Route::ExplanationInput { segment_id } => {
                explanation_input_view::show(ctx, &self.state, &segment_id, &mut self.explanation_form, &mut self.route);
            }
            Route::Settings => {
                settings_view::show(ctx, &self.state, &mut self.route, &mut self.settings_tab, &mut self.settings_ui);
            }
            _ => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(BG))
                    .show(ctx, |ui| match self.route.clone() {
                        Route::Login => {
                            login_view::ui(ui, &self.state, &mut self.login_form, &mut self.route)
                        }
                        Route::UpdateNotice => {
                            update_view::ui(ui, &self.state, &mut self.route)
                        }
                        Route::Disabled => {
                            disabled_view::ui(ui, &self.state, &mut self.route)
                        }
                        _ => {}
                    });
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

/// 서브페이지 공용 오렌지 헤더 패널 (뒤로가기 + 페이지 제목).
/// back_route 클릭 시 route 를 그 값으로 변경.
pub fn orange_header(
    ctx: &egui::Context,
    title: &str,
    back_label: &str,
    route: &mut Route,
    back_route: Route,
) {
    egui::TopBottomPanel::top("page_header")
        .frame(
            egui::Frame::none()
                .fill(ORANGE)
                .inner_margin(egui::Margin::symmetric(24.0, 14.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // 뒤로가기
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("‹  {back_label}"))
                                .size(15.0)
                                .color(egui::Color32::from_rgba_premultiplied(255, 255, 255, 200)),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    *route = back_route;
                }

                ui.add_space(12.0);

                // 구분선
                ui.painter().line_segment(
                    [
                        ui.cursor().min + egui::vec2(0.0, 2.0),
                        ui.cursor().min + egui::vec2(0.0, 22.0),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 80)),
                );

                ui.add_space(12.0);

                // 페이지 제목
                ui.label(
                    egui::RichText::new(title)
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
            });
        });
}

/// 공통 비주얼 — 라이트 테마 + 둥근 모서리 + 패딩.
fn setup_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = BG;
    visuals.window_fill = egui::Color32::WHITE;
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
    visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
    visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
    visuals.widgets.active.rounding = egui::Rounding::same(8.0);
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 8.0);
    });
}

/// 앱 시작 직후 한 번 호출 — 백그라운드에서 자동로그인 시도.
/// PIN+ 미사용은 `subscription_service` 에 false 가 저장되며, 메인 화면 팝업이
/// 사용자에게 안내 — 본 함수에서 별도 처리 불필요.
fn kick_auto_login(state: Arc<AppState>) {
    use crate::domain::usecase::user_usecase;
    let runtime = state.runtime.clone();
    runtime.spawn(async move {
        // UI 캐시 사전 비움 — 부팅 직후라 사실상 빈 상태이지만 일관성 차원에서 명시.
        crate::ui::explanation_list_view::clear_cache();
        match user_usecase::auto_login(&state).await {
            Ok(Some(_)) => info!("자동로그인 성공"),
            Ok(None) => info!("자동로그인 대상 없음"),
            Err(e) => tracing::warn!(error = %e, "자동로그인 처리 실패"),
        }
    });
}

/// OS 별 한글 폰트 파일을 찾아 egui font 로 등록.
/// 파일을 못 찾으면 egui 기본 폰트만 — 한글이 □ 로 깨질 수 있음.
fn install_korean_font(ctx: &egui::Context) {
    let candidates: &[&str] = if cfg!(windows) {
        &["C:/Windows/Fonts/malgun.ttf", "C:/Windows/Fonts/malgunbd.ttf"]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            "/Library/Fonts/AppleSDGothicNeo.ttc",
        ]
    } else {
        &["/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"]
    };

    let mut bytes: Option<Vec<u8>> = None;
    for path in candidates {
        if let Ok(b) = std::fs::read(path) {
            bytes = Some(b);
            break;
        }
    }
    let Some(data) = bytes else { return };
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("kr".to_string(), FontData::from_owned(data));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "kr".to_string());
    fonts.families.entry(FontFamily::Monospace).or_default().push("kr".to_string());
    ctx.set_fonts(fonts);
}
