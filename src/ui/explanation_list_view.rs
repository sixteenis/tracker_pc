//! ============================================================================
//! ui::explanation_list_view — 근무시간 소명 대상 목록 화면.
//! ============================================================================
//!
//! 본 화면 진입 경로:
//!   - 상단 탭 "근무시간 소명"
//!   - 상태 화면의 "오늘 기록 보기" / 소명 N건 진입 버튼
//!   - 토스트 (현재는 클릭 콜백 미구현 — 사용자가 직접 진입)
//!
//! 표시: 날짜 / 시작 / 종료 / 간격 / 적용 기준 / 종류 / 상태 / 소명 마감 + 소명하기 버튼.
//! 첫 진입 시 30분 안에 한 번만 토스트로 알림.
//!
//! TODO(2차): 서버측 segment (`api::list_explanations`) 와 로컬 segment 병합 표시.
//! 현재는 로컬 only — 다른 PC 에서 만든 segment 가 안 보임.
//! TODO(2차): 빈 상태에서 "테스트 자리비움 1건 생성" 버튼은 Mock 모드 디버깅 용.
//! 실서버 연결 시 자동 숨김 (이미 `if state.config.api.mock_mode` 조건 들어가 있음).

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use eframe::egui;

use crate::app::AppState;
use crate::db::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::ui::Route;
use crate::util;

/// 화면 진입점 — 헤더 + 콘텐츠.
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

    // 토스트는 여기서 띄우지 않는다.
    // - 사용자가 이미 목록 화면을 보고 있으므로 알림이 중복.
    // - macOS 의 unbundled 빌드에서 notify-rust 가 LaunchServices 다이얼로그를
    //   띄우는 부작용("Choose Application — Where is …?")이 있어 화면 전환을 가린다.
    // 토스트가 진짜 의미 있는 순간은 백그라운드에서 새 자리비움이 막 발생했을 때 —
    // 그 시점에는 `monitor::idle_detector::open_segment` 가 별도 thread 에서 호출.
    let _ = last_toast_at; // 변수 미사용 경고 억제 (시그니처 호환 유지)

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
/// 실서버 모드에서는 이 함수가 호출되지 않음 (UI 가드).
/// TODO(2차 정리): 정식 배포 전 본 함수와 호출 UI 제거.
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
