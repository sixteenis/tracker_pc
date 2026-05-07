mod disabled_view;
mod explanation_input_view;
mod explanation_list_view;
mod login_view;
pub mod settings_view;
mod status_view;
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
    ExplanationList,
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
}

impl PinpleApp {
    pub fn new(cc: &CreationContext<'_>, state: Arc<AppState>) -> Self {
        install_korean_font(&cc.egui_ctx);
        setup_visuals(&cc.egui_ctx);

        let initial_route = if state.is_logged_in() {
            if state.can_track_time() { Route::Status } else { Route::Disabled }
        } else {
            kick_auto_login(state.clone());
            Route::Login
        };
        Self {
            state,
            route: initial_route,
            login_form: login_view::LoginForm::default(),
            explanation_form: explanation_input_view::ExplanationForm::default(),
            last_toast_at: None,
            settings_tab: settings_view::SettingsTab::General,
            settings_ui: settings_view::SettingsUi::default(),
        }
    }
}

impl App for PinpleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if matches!(self.route, Route::Login) && self.state.is_logged_in() {
            self.route = if self.state.can_track_time() { Route::Status } else { Route::Disabled };
        }

        match self.route.clone() {
            Route::Status => {
                status_view::show(ctx, &self.state, &mut self.route);
            }
            Route::ExplanationList => {
                explanation_list_view::show(ctx, &self.state, &mut self.route, &mut self.last_toast_at);
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

/// 서브페이지 공용 오렌지 헤더 패널.
/// back_route 클릭 시 route 를 변경하고 true 를 반환.
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

fn kick_auto_login(state: Arc<AppState>) {
    let runtime = state.runtime.clone();
    runtime.spawn(async move {
        match crate::auth::try_auto_login(&state).await {
            Ok(Some(())) => info!("자동로그인 성공"),
            Ok(None) => info!("자동로그인 대상 없음"),
            Err(e) => tracing::warn!(error = %e, "자동로그인 처리 실패"),
        }
    });
}

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
