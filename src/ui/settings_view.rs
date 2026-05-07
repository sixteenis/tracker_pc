use std::sync::Arc;

use eframe::egui;

use crate::app::AppState;
use crate::ui::{GRAY_TEXT, NAVY, ORANGE, Route};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Detection,
    Account,
}

pub struct SettingsUi {
    pub auto_start: bool,
    pub notifications: bool,
    pub tray_icon: bool,
}

impl Default for SettingsUi {
    fn default() -> Self {
        Self { auto_start: true, notifications: true, tray_icon: true }
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    route: &mut Route,
    tab: &mut SettingsTab,
    settings: &mut SettingsUi,
) {
    crate::ui::orange_header(ctx, "환경설정", "메인", route, Route::Status);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::WHITE))
        .show(ctx, |ui| {
            content(ui, state, route, tab, settings);
        });
}

fn content(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    route: &mut Route,
    tab: &mut SettingsTab,
    settings: &mut SettingsUi,
) {
    let pad = 28.0;

    // ── SETTINGS 헤더 ─────────────────────────────────────────────
    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("SETTINGS").size(11.0).color(ORANGE).strong());
            ui.add_space(2.0);
            ui.label(egui::RichText::new("환경설정").size(26.0).color(NAVY).strong());
        });
    });

    ui.add_space(18.0);

    // ── 탭 바 ─────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(pad);
        for (label, t) in [
            ("일반", SettingsTab::General),
            ("감지 설정", SettingsTab::Detection),
            ("계정", SettingsTab::Account),
        ] {
            let active = *tab == t;
            let text_color = if active { ORANGE } else { GRAY_TEXT };
            let btn = egui::Button::new(
                egui::RichText::new(label).size(14.0).color(text_color).strong(),
            )
            .frame(false);
            let resp = ui.add(btn);
            if resp.clicked() {
                *tab = t;
            }
            // 선택된 탭 밑줄
            if active {
                let underline_rect = egui::Rect::from_min_size(
                    resp.rect.left_bottom() + egui::vec2(0.0, 2.0),
                    egui::vec2(resp.rect.width(), 2.5),
                );
                ui.painter().rect_filled(underline_rect, egui::Rounding::ZERO, ORANGE);
            }
            ui.add_space(20.0);
        }
    });

    ui.add_space(2.0);

    // 탭 구분선
    let sep_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        egui::vec2(ui.available_width(), 1.0),
    );
    ui.painter().rect_filled(sep_rect, egui::Rounding::ZERO, egui::Color32::from_rgb(230, 230, 230));
    ui.add_space(1.0);

    // ── 탭 콘텐츠 ─────────────────────────────────────────────────
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        match tab {
            SettingsTab::General => {
                toggle_row(
                    ui,
                    pad,
                    "윈도우 시작 시 자동 실행",
                    "PC 부팅 시 백그라운드로 자동 실행됩니다",
                    &mut settings.auto_start,
                );
                toggle_row(
                    ui,
                    pad,
                    "알림 표시",
                    "이탈 기록·동기화 상태 알림",
                    &mut settings.notifications,
                );
                toggle_row(
                    ui,
                    pad,
                    "트레이 아이콘",
                    "작업 표시줄에 상태 아이콘 표시",
                    &mut settings.tray_icon,
                );
                policy_row(
                    ui,
                    pad,
                    "프로그램 종료 허용",
                    "관리자가 종료 권한을 제어합니다",
                    "관리자 정책",
                    "허용됨",
                );
            }
            SettingsTab::Detection => {
                let snapshot = state.snapshot_status();
                let policy = state.snapshot_policy();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        info_row(ui, "자리비움 기준 시간", &format!("{}분 ({})", snapshot.effective_idle_threshold_seconds / 60, snapshot.policy_scope));
                        info_row(ui, "정책 버전", &policy.policy_version.to_string());
                        info_row(ui, "점심 시간", &format!("{} ~ {} ({}분 인정)", policy.lunch_start_time, policy.lunch_end_time, policy.lunch_allowed_minutes));
                        info_row(ui, "소명 마감 기한", &format!("{}시간 이내", policy.explanation_deadline_hours));
                    });
                });
            }
            SettingsTab::Account => {
                let session = state.session.read().unwrap().clone();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        if let Some(sess) = &session {
                            info_row(ui, "이름", sess.employee_name.as_deref().unwrap_or("—"));
                            info_row(ui, "사원 ID", &sess.employee_id);
                            info_row(ui, "회사 ID", &sess.company_id);
                            info_row(ui, "팀", sess.team_name.as_deref().unwrap_or("—"));
                            info_row(ui, "기기 ID", &state.device.device_id);
                            info_row(ui, "기기명", &state.device.device_name);
                            info_row(ui, "앱 버전", &state.config.app.app_version);
                        } else {
                            ui.label(egui::RichText::new("로그인 정보 없음").color(GRAY_TEXT));
                        }
                        ui.add_space(16.0);
                        let logout_btn = egui::Button::new(
                            egui::RichText::new("로그아웃").size(14.0).color(egui::Color32::from_rgb(210, 50, 50)),
                        )
                        .fill(egui::Color32::from_rgb(255, 240, 240))
                        .rounding(egui::Rounding::same(8.0))
                        .min_size(egui::vec2(120.0, 38.0));
                        if ui.add(logout_btn).clicked() {
                            let _ = crate::auth::logout(state);
                            *route = Route::Login;
                        }
                    });
                });
            }
        }

        ui.add_space(24.0);
    });
}

// ── 토글 스위치 위젯 ──────────────────────────────────────────

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) {
    let size = egui::vec2(46.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let painter = ui.painter_at(rect);
    let t = ui.ctx().animate_bool(resp.id, *on);
    let bg = egui::Color32::from_rgb(
        (200.0 - 170.0 * t) as u8,
        (200.0 - 132.0 * t) as u8,
        (200.0 - 168.0 * t) as u8,
    );
    painter.rect_filled(rect, egui::Rounding::same(rect.height() / 2.0), bg);
    let cx = rect.left() + rect.height() / 2.0 + t * (rect.width() - rect.height());
    painter.circle_filled(
        egui::pos2(cx, rect.center().y),
        rect.height() / 2.0 - 2.5,
        egui::Color32::WHITE,
    );
}

// ── 설정 행 ───────────────────────────────────────────────────

fn toggle_row(ui: &mut egui::Ui, pad: f32, title: &str, desc: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 70.0);
            ui.label(egui::RichText::new(title).size(15.0).color(NAVY).strong());
            ui.label(egui::RichText::new(desc).size(12.0).color(GRAY_TEXT));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(pad);
            toggle_switch(ui, value);
        });
    });
    ui.add_space(4.0);
    let sep = egui::Rect::from_min_size(
        ui.cursor().min + egui::vec2(pad, 0.0),
        egui::vec2(ui.available_width() - pad, 1.0),
    );
    ui.painter().rect_filled(sep, egui::Rounding::ZERO, egui::Color32::from_rgb(235, 235, 235));
    ui.add_space(1.0);
    ui.add_space(8.0);
}

fn policy_row(ui: &mut egui::Ui, pad: f32, title: &str, desc: &str, badge: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).size(15.0).color(NAVY).strong());
            ui.add_space(4.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(230, 235, 255))
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(badge).size(10.0).color(egui::Color32::from_rgb(60, 80, 180)));
                });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(pad);
            ui.label(egui::RichText::new(value).size(13.0).color(egui::Color32::from_rgb(30, 180, 120)).strong());
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.label(egui::RichText::new(desc).size(12.0).color(GRAY_TEXT));
    });
    ui.add_space(4.0);
    let sep = egui::Rect::from_min_size(
        ui.cursor().min + egui::vec2(pad, 0.0),
        egui::vec2(ui.available_width() - pad, 1.0),
    );
    ui.painter().rect_filled(sep, egui::Rounding::ZERO, egui::Color32::from_rgb(235, 235, 235));
    ui.add_space(1.0);
    ui.add_space(8.0);
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(13.0).color(GRAY_TEXT));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(value).size(13.0).color(NAVY).strong());
    });
    ui.add_space(6.0);
}
