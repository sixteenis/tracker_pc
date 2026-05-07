//! 근무시간 소명 목록 화면.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use eframe::egui;

use crate::app::AppState;
use crate::db::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::ui::Route;
use crate::util;

pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    route: &mut Route,
    last_toast_at: &mut Option<DateTime<Utc>>,
) {
    crate::ui::orange_header(ctx, "근무시간 소명", "메인", route, Route::Status);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(crate::ui::BG).inner_margin(egui::Margin::same(20.0)))
        .show(ctx, |ui| {
            content(ui, state, route, last_toast_at);
        });
}

fn content(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    route: &mut Route,
    last_toast_at: &mut Option<DateTime<Utc>>,
) {
    ui.label(
        egui::RichText::new(
            "근무시간 중 PC 미사용 시간이 감지되었습니다. 업무상 사유가 있는 경우 소명해 주세요.",
        )
        .size(12.0)
        .color(crate::ui::GRAY_TEXT),
    );
    ui.add_space(6.0);
    ui.separator();

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

    if segments.is_empty() {
        ui.add_space(20.0);
        ui.colored_label(
            egui::Color32::GRAY,
            "소명이 필요한 자리비움 구간이 없습니다.",
        );
        ui.label(
            "자리비움 구간은 PC 미사용 시간이 회사 정책의 기준 시간을 초과하면 자동으로 생성됩니다.\n\
             현재 회사 정책 기준 시간은 상태 화면에서 확인할 수 있습니다.",
        );
        ui.add_space(16.0);
        if state.config.api.mock_mode {
            ui.separator();
            ui.colored_label(egui::Color32::LIGHT_BLUE, "ℹ Mock 모드 — 화면 동작 확인용");
            if ui.button("테스트 자리비움 1건 생성").clicked() {
                if let Err(e) = seed_test_segment(state, &session) {
                    tracing::warn!(error = %e, "테스트 segment 생성 실패");
                }
            }
        }
        return;
    }

    // 가장 최근 한 건에 대해 한번만 토스트 알림.
    let latest = &segments[0];
    let now = Utc::now();
    let should_toast = last_toast_at
        .map(|t| (now - t).num_minutes() >= 30)
        .unwrap_or(true);
    if should_toast {
        if let Some(end) = latest.end_time {
            let mins = (end - latest.start_time).num_minutes().max(0);
            let _ = crate::notify::show_explanation_request(
                &util::format_local_time(&latest.start_time),
                &util::format_local_time(&end),
                mins,
            );
        }
        *last_toast_at = Some(now);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("explanation_grid").num_columns(9).striped(true).show(ui, |ui| {
            ui.strong("날짜");
            ui.strong("시작");
            ui.strong("종료");
            ui.strong("간격");
            ui.strong("적용 기준");
            ui.strong("종류");
            ui.strong("상태");
            ui.strong("소명 마감");
            ui.strong("");
            ui.end_row();

            for seg in &segments {
                ui.label(seg.work_date.format("%Y-%m-%d").to_string());
                ui.label(util::format_local_time(&seg.start_time));
                ui.label(seg.end_time.as_ref().map(util::format_local_time).unwrap_or_else(|| "진행 중".into()));
                ui.label(util::format_duration_human(seg.duration_seconds.unwrap_or(0)));
                ui.label(format!(
                    "{} / {}",
                    util::format_duration_human(seg.applied_idle_threshold_seconds),
                    seg.policy_scope
                ));
                ui.label(seg.segment_type.as_str());
                ui.label(seg.explanation_status.as_str());
                ui.label(
                    seg.explanation_deadline
                        .as_ref()
                        .map(util::format_local_dt)
                        .unwrap_or_else(|| "—".into()),
                );
                if ui.button("소명하기").clicked() {
                    *route = Route::ExplanationInput { segment_id: seg.segment_id.clone() };
                }
                ui.end_row();
            }
        });
    });
}

/// Mock 모드 전용 — 화면 검증을 위해 5분 짜리 자리비움 구간 1건을 즉시 생성.
fn seed_test_segment(state: &Arc<AppState>, session: &crate::auth::Session) -> anyhow::Result<()> {
    let now = Utc::now();
    let start = now - Duration::minutes(5);
    let snapshot = state.snapshot_status();
    let policy = state.snapshot_policy();
    let new_seg = NewSegment {
        company_id: session.company_id.clone(),
        employee_id: session.employee_id.clone(),
        device_id: state.device.device_id.clone(),
        work_date: now.date_naive(),
        segment_type: SegmentType::PcIdle,
        start_time: start,
        end_time: Some(now),
        applied_idle_threshold_seconds: snapshot.effective_idle_threshold_seconds as i64,
        policy_scope: snapshot.policy_scope.clone(),
        explanation_deadline: Some(
            now + Duration::hours(policy.explanation_deadline_hours.max(1) as i64),
        ),
    };
    let segment_id = idle_segments_repo::insert(&state.db, &new_seg)?;
    crate::db::events_repo::enqueue(
        &state.db,
        "IDLE_STARTED",
        now,
        &serde_json::json!({
            "segment_id": segment_id,
            "started_at": start.to_rfc3339(),
            "applied_idle_threshold_seconds": snapshot.effective_idle_threshold_seconds,
            "policy_scope": snapshot.policy_scope,
            "synthetic": true,
        }),
    )?;
    Ok(())
}
