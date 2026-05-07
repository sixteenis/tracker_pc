//! 근무시간 소명 입력 화면.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::api::types::{ExplanationSubmit, ExplanationType};
use crate::app::AppState;
use crate::db::explanations_repo::{self, NewExplanation};
use crate::db::idle_segments_repo;
use crate::ui::Route;
use crate::util;

pub struct ExplanationForm {
    pub explanation_type: ExplanationType,
    pub explanation_text: String,
    pub status: Arc<Mutex<Option<String>>>,
}

impl Default for ExplanationForm {
    fn default() -> Self {
        Self {
            explanation_type: ExplanationType::Meeting,
            explanation_text: String::new(),
            status: Arc::new(Mutex::new(None)),
        }
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    segment_id: &str,
    form: &mut ExplanationForm,
    route: &mut Route,
) {
    crate::ui::orange_header(ctx, "소명 작성", "목록", route, Route::ExplanationList);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(crate::ui::BG).inner_margin(egui::Margin::same(20.0)))
        .show(ctx, |ui| {
            content(ui, state, segment_id, form, route);
        });
}

fn content(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    segment_id: &str,
    form: &mut ExplanationForm,
    route: &mut Route,
) {
    let session = match state.session.read().unwrap().clone() {
        Some(s) => s,
        None => {
            ui.colored_label(egui::Color32::YELLOW, "로그인이 필요합니다.");
            return;
        }
    };

    let segments = match idle_segments_repo::list_pending_for_employee(&state.db, &session.employee_id) {
        Ok(s) => s,
        Err(e) => {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("조회 실패: {e}"));
            return;
        }
    };
    let seg = match segments.into_iter().find(|s| s.segment_id == segment_id) {
        Some(s) => s,
        None => {
            ui.label("선택한 자리비움 구간을 찾을 수 없습니다.");
            if ui.button("목록으로").clicked() {
                *route = Route::ExplanationList;
            }
            return;
        }
    };

    ui.separator();
    egui::Grid::new("seg_meta").num_columns(2).show(ui, |ui| {
        ui.label("날짜");
        ui.label(seg.work_date.format("%Y-%m-%d").to_string());
        ui.end_row();
        ui.label("시작 시간");
        ui.label(util::format_local_time(&seg.start_time));
        ui.end_row();
        ui.label("종료 시간");
        ui.label(seg.end_time.as_ref().map(util::format_local_time).unwrap_or_else(|| "진행 중".into()));
        ui.end_row();
        ui.label("자리비움 간격");
        ui.label(util::format_duration_human(seg.duration_seconds.unwrap_or(0)));
        ui.end_row();
        ui.label("적용된 기준 시간");
        ui.label(format!(
            "{} ({})",
            util::format_duration_human(seg.applied_idle_threshold_seconds),
            seg.policy_scope
        ));
        ui.end_row();
    });

    ui.separator();
    ui.label("소명 사유");
    egui::ComboBox::from_id_salt("explanation_type")
        .selected_text(form.explanation_type.label())
        .show_ui(ui, |ui| {
            for &t in ExplanationType::ALL {
                ui.selectable_value(&mut form.explanation_type, t, t.label());
            }
        });

    ui.add_space(6.0);
    ui.label("소명 내용");
    ui.add(
        egui::TextEdit::multiline(&mut form.explanation_text)
            .desired_rows(5)
            .desired_width(f32::INFINITY),
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("취소").clicked() {
            *route = Route::ExplanationList;
        }
        if ui.button("제출").clicked() {
            submit(state, &seg, form);
            *route = Route::ExplanationList;
        }
    });

    if let Ok(s) = form.status.lock() {
        if let Some(msg) = s.as_ref() {
            ui.add_space(8.0);
            ui.label(msg);
        }
    }
}

fn submit(state: &Arc<AppState>, seg: &idle_segments_repo::IdleSegment, form: &mut ExplanationForm) {
    let new = NewExplanation {
        segment_id: seg.segment_id.clone(),
        work_date: seg.work_date,
        start_time: seg.start_time,
        end_time: seg.end_time.unwrap_or(seg.start_time),
        duration_seconds: seg.duration_seconds.unwrap_or(0),
        explanation_type: form.explanation_type.code().to_string(),
        explanation_text: if form.explanation_text.trim().is_empty() {
            None
        } else {
            Some(form.explanation_text.trim().to_string())
        },
    };

    let local_id = match explanations_repo::insert(&state.db, &new) {
        Ok(id) => id,
        Err(e) => {
            if let Ok(mut s) = form.status.lock() {
                *s = Some(format!("로컬 저장 실패: {e}"));
            }
            return;
        }
    };
    let _ = idle_segments_repo::mark_submitted(&state.db, &seg.segment_id);
    form.explanation_text.clear();

    // 비동기로 서버 전송.
    let state2 = state.clone();
    let payload = ExplanationSubmit {
        segment_id: new.segment_id.clone(),
        explanation_type: new.explanation_type.clone(),
        explanation_text: new.explanation_text.clone(),
        submitted_from: "PC_APP".to_string(),
    };
    let status_slot = form.status.clone();
    state.runtime.spawn(async move {
        let maybe_session = state2.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => return,
        };
        match state2.api.submit_explanation(&session.access_token, payload).await {
            Ok(()) => {
                let _ = explanations_repo::mark_synced(&state2.db, local_id);
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some("제출 완료".to_string());
                }
            }
            Err(e) => {
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some(format!("서버 전송 실패 — 로컬에 저장됨: {e}"));
                }
            }
        }
    });
}
