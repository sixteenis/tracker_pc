//! ============================================================================
//! ui::disabled_view — 요금제 비활성 안내 화면 (기획서 §7).
//! ============================================================================
//!
//! `subscription.can_track_time = false` 또는 `policy.can_track_time = false` 일 때
//! 강제 표시. 로그인은 유지하되 idle 감지/heartbeat 모두 멈춘 상태.
//!
//! TODO(자동 복귀): heartbeat 응답으로 can_track_time 가 true 로 바뀌면 자동으로
//! `Route::Status` 로 가는 분기 미구현. 현재는 사용자가 로그아웃 후 재로그인해야
//! 복귀. UI mod 의 update() 에 자동 라우팅 분기 추가 필요.

use std::sync::Arc;

use eframe::egui;

use crate::app::AppState;
use crate::ui::Route;

pub fn ui(ui: &mut egui::Ui, state: &Arc<AppState>, route: &mut Route) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.heading("PC 근무활동 기록 기능이 비활성화되어 있습니다");
        ui.add_space(8.0);
        ui.label("현재 회사 요금제에서는 이 기능을 사용할 수 없습니다.");
        ui.label("관리자에게 문의해 주세요.");
        ui.add_space(20.0);
        if state.is_logged_in() {
            if ui.button("설정/정보").clicked() {
                *route = Route::Settings;
            }
            if ui.button("로그아웃").clicked() {
                let _ = crate::domain::service::user_service::logout(state);
                *route = Route::Login;
            }
        } else if ui.button("로그인 화면으로").clicked() {
            *route = Route::Login;
        }
    });
}
