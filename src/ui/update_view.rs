//! ============================================================================
//! ui::update_view — 업데이트 안내 화면.
//! ============================================================================
//!
//! `update_info` 가 채워졌을 때만 의미 있는 내용 표시. force_update 일 때 PC 기록
//! 기능이 즉시 비활성됨 (sync::update_check 가 처리).
//!
//! TODO(미구현): `download_url` 을 클릭 가능한 하이퍼링크로만 제공. 자동 다운로드/
//! 실행은 안 함. `sync::update_check` 의 TODO 참고.
//! TODO(2차): 릴리즈 노트 마크다운 렌더링 (egui 가 기본은 plain text).

use std::sync::Arc;

use eframe::egui;

use crate::app::AppState;
use crate::ui::Route;

pub fn ui(ui: &mut egui::Ui, state: &Arc<AppState>, route: &mut Route) {
    ui.heading("앱 업데이트");
    let info = state.update_info.read().unwrap().clone();
    match info {
        Some(info) => {
            ui.label(format!("현재 버전: {}", info.current_version));
            ui.label(format!("최신 버전: {}", info.latest_version));
            ui.label(format!("필수 최소 버전: {}", info.minimum_required_version));
            ui.label(format!(
                "업데이트 필요: {}{}",
                if info.update_required { "예" } else { "아니오" },
                if info.force_update { " (강제)" } else { "" }
            ));
            ui.add_space(8.0);
            ui.label("릴리즈 노트");
            ui.label(&info.release_note);
            if !info.download_url.is_empty() {
                ui.add_space(8.0);
                ui.hyperlink_to("다운로드 페이지 열기", &info.download_url);
            }
        }
        None => {
            ui.label("아직 업데이트 정보를 받지 못했습니다.");
        }
    }
    ui.add_space(12.0);
    if ui.button("뒤로").clicked() {
        *route = Route::Status;
    }
}
