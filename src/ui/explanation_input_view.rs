//! ============================================================================
//! ui::explanation_input_view — 자리비움 한 건에 대한 소명 입력 화면.
//! ============================================================================
//!
//! 진입: 목록 화면 각 행의 "소명하기" 버튼 → `Route::ExplanationInput { segment_id }`.
//! 표시: segment 메타 (날짜/시작/종료/간격/적용 기준)
//! 입력: 사유 콤보 (회사 커스텀 동적 목록, 캐시 미스 시 시스템 기본 12개 fallback) + 자유 텍스트
//! 제출: 로컬 DB insert → segment status SUBMITTED 마크 → 비동기 서버 POST.
//!
//! Phase 1.b (2026-05-12): `ExplanationType` enum 폐기. 사유 목록은
//! `domain::service::explanation_type_service::current_types()` 로 동기 조회.
//! 서버 거부(`400 INVALID_EXPLANATION_TYPE`) 시 안내 메시지 + 사용자가 화면 재진입하면
//! 다음 user-info 폴링이 캐시 갱신.
//!
//! TODO(미구현): 서버 전송 실패 시 자동 재시도 워커 없음. 현재는 입력 화면에서
//! 1회만 전송 시도. `db::explanations_repo` 헤더의 TODO 참조.
//! TODO(2차): 임시 저장 — 사용자가 입력 도중 화면 떠나면 텍스트 유지.
//! TODO(2차): 사유에 따라 추가 필드 (예: BUSINESS_TRIP 이면 출장지) 동적 표시.

use std::sync::{Arc, Mutex};

use eframe::egui;

use chrono::{DateTime, NaiveDate, Utc};

use crate::data::dto::{ExplanationSubmit, RemoteExplanation};
use crate::app::AppState;
use crate::constants;
use crate::data::local::explanations_repo::{self, NewExplanation};
use crate::data::local::idle_segments_repo::{self, ExplanationStatus, IdleSegment, SegmentType};
use crate::domain::model::user::User;
use crate::domain::service::explanation_type_service;
use crate::ui::Route;
use crate::util;

/// "기타" (code='OTHER') 선택 시 사용자가 직접 입력하는 유형명의 최대 길이 (UTF-8 chars).
/// 서버 검증과 일치. `PCAGT_EXPLANATION.OTHER_TYPE_LABEL NVARCHAR(50)` 와 동기.
const OTHER_TYPE_LABEL_MAX: usize = 50;

/// "기타" 사유의 약속된 코드. 서버 시드 + DB 룩업과 동일.
const OTHER_TYPE_CODE: &str = "OTHER";

pub struct ExplanationForm {
    /// 사용자가 콤보에서 선택한 사유 코드 (예: "MEETING"). 빈 문자열이면 미선택.
    pub selected_code: String,
    pub explanation_text: String,
    /// `selected_code == "OTHER"` 일 때만 사용 — 사용자가 직접 입력한 유형명.
    pub other_type_label: String,
    pub status: Arc<Mutex<Option<String>>>,
}

impl Default for ExplanationForm {
    fn default() -> Self {
        Self {
            selected_code: String::new(),
            explanation_text: String::new(),
            other_type_label: String::new(),
            status: Arc::new(Mutex::new(None)),
        }
    }
}

/// 화면 진입점 — 오렌지 헤더 + 콘텐츠.
pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    segment_id: &str,
    form: &mut ExplanationForm,
    route: &mut Route,
) {
    crate::ui::orange_header(ctx, "소명 작성", "목록", route, Route::ExplanationList { today_only: false });
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

    // 1차: 로컬 SQLite idle_segments 에서 조회 (PENDING/EXPIRED 만).
    // 2차: 로컬 부재 또는 status 가 PENDING/EXPIRED 가 아닌 경우 서버 캐시 (목록 화면)
    //      에서 단건 조회. 다른 PC 에서 만든 segment / SUBMITTED 마킹된 row 도 잡힘.
    //      (2026-05-13 핫픽스 — list view 는 서버 응답 진실 소스인데 input view 만 로컬 의존이었음)
    let local_seg = idle_segments_repo::list_pending_for_employee(&state.db, &session.employee_id_str)
        .ok()
        .and_then(|segs| segs.into_iter().find(|s| s.segment_id == segment_id));
    let seg = match local_seg {
        Some(s) => s,
        None => match crate::ui::explanation_list_view::find_in_cache(segment_id) {
            Some(remote) => remote_to_idle_segment(remote, &session, &state.device.device_id),
            None => {
                ui.label("선택한 자리비움 구간을 찾을 수 없습니다.");
                if ui.button("목록으로").clicked() {
                    *route = Route::ExplanationList { today_only: false };
                }
                return;
            }
        },
    };

    let tz_offset = state.snapshot_policy().time_zone_offset_minutes;
    ui.separator();
    egui::Grid::new("seg_meta").num_columns(2).show(ui, |ui| {
        ui.label("날짜");
        ui.label(seg.work_date.format("%Y-%m-%d").to_string());
        ui.end_row();
        ui.label("시작 시간");
        ui.label(util::format_company_time(&seg.start_time, tz_offset));
        ui.end_row();
        ui.label("종료 시간");
        ui.label(seg.end_time.as_ref().map(|t| util::format_company_time(t, tz_offset)).unwrap_or_else(|| "진행 중".into()));
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
    // 회사 커스텀 동적 사유 목록 (캐시 미스 시 시스템 기본 12개 fallback).
    let types = explanation_type_service::current_types();
    if form.selected_code.is_empty() {
        if let Some(first) = types.first() {
            form.selected_code = first.code.clone();
        }
    }
    let selected_label = types
        .iter()
        .find(|t| t.code == form.selected_code)
        .map(|t| t.label.clone())
        .unwrap_or_else(|| "선택".to_string());
    egui::ComboBox::from_id_salt("explanation_type")
        .selected_text(selected_label)
        .show_ui(ui, |ui| {
            for t in &types {
                ui.selectable_value(&mut form.selected_code, t.code.clone(), &t.label);
            }
        });

    // requires_text 인 사유는 자유 텍스트 강제 — 빈 입력 시 제출 비활성화 + 안내.
    let requires_text = types
        .iter()
        .find(|t| t.code == form.selected_code)
        .map(|t| t.requires_text)
        .unwrap_or(false);

    let is_other = form.selected_code == OTHER_TYPE_CODE;

    if is_other {
        ui.add_space(6.0);
        ui.label("기타 유형명 (필수, 최대 50자)");
        ui.add(
            egui::TextEdit::singleline(&mut form.other_type_label)
                .char_limit(OTHER_TYPE_LABEL_MAX)
                .desired_width(f32::INFINITY)
                .hint_text("예) 외부 거래처 회식"),
        );
    }
    let other_label_trimmed = form.other_type_label.trim();
    let other_label_missing = is_other && other_label_trimmed.is_empty();
    if other_label_missing {
        ui.colored_label(
            egui::Color32::from_rgb(210, 50, 50),
            "기타 유형명을 1자 이상 입력해 주세요.",
        );
    }

    ui.add_space(6.0);
    ui.label(if requires_text {
        "소명 내용 (필수)"
    } else {
        "소명 내용"
    });
    ui.add(
        egui::TextEdit::multiline(&mut form.explanation_text)
            .desired_rows(5)
            .desired_width(f32::INFINITY),
    );

    let text_missing = requires_text && form.explanation_text.trim().is_empty();
    if text_missing {
        ui.colored_label(
            egui::Color32::from_rgb(210, 50, 50),
            "이 사유는 소명 내용을 반드시 입력해야 합니다.",
        );
    }

    let can_submit = !text_missing && !other_label_missing;
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("취소").clicked() {
            *route = Route::ExplanationList { today_only: false };
        }
        let submit_btn = ui.add_enabled(can_submit, egui::Button::new("제출"));
        if submit_btn.clicked() {
            submit(state, &seg, form);
            *route = Route::ExplanationList { today_only: false };
        }
    });

    if let Ok(s) = form.status.lock() {
        if let Some(msg) = s.as_ref() {
            ui.add_space(8.0);
            ui.label(msg);
        }
    }
}

/// 제출 버튼 클릭. 로컬 DB 저장 + segment SUBMITTED + 비동기 서버 전송.
/// 서버 실패해도 로컬에는 남아 다음에 다시 보낼 수 있음 (재시도는 미구현).
fn submit(state: &Arc<AppState>, seg: &idle_segments_repo::IdleSegment, form: &mut ExplanationForm) {
    let (employee_id_str, company_id_str, employee_id) = match state.session.read().unwrap().clone() {
        Some(s) => (s.employee_id_str, s.company_id_str, s.employee_id),
        None => {
            if let Ok(mut s) = form.status.lock() {
                *s = Some("로그인이 필요합니다.".to_string());
            }
            return;
        }
    };
    let device_id = state.device.device_id.clone();

    let other_type_label = if form.selected_code == OTHER_TYPE_CODE {
        let trimmed = form.other_type_label.trim();
        if trimmed.is_empty() {
            // UI 가드(제출 버튼 비활성)로 거의 도달 불가. 안전 차단.
            if let Ok(mut s) = form.status.lock() {
                *s = Some("기타 유형명을 입력해 주세요.".to_string());
            }
            return;
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    let new = NewExplanation {
        segment_id: seg.segment_id.clone(),
        work_date: seg.work_date,
        start_time: seg.start_time,
        end_time: seg.end_time.unwrap_or(seg.start_time),
        duration_seconds: seg.duration_seconds.unwrap_or(0),
        explanation_type: form.selected_code.clone(),
        explanation_text: if form.explanation_text.trim().is_empty() {
            None
        } else {
            Some(form.explanation_text.trim().to_string())
        },
        other_type_label: other_type_label.clone(),
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
    form.other_type_label.clear();

    // 옵티미스틱: 목록 화면 캐시의 해당 segment 를 즉시 SUBMITTED 로 전환 →
    // 라우트 복귀 직후 "소명 필요" 탭에서 바로 사라진다. 서버 응답으로 자동 동기화.
    crate::ui::explanation_list_view::mark_submitted_optimistic(&seg.segment_id);

    // 비동기로 서버 전송.
    let state2 = state.clone();
    let payload = ExplanationSubmit {
        employee_id: employee_id_str,
        company_id: company_id_str,
        device_id,
        segment_id: new.segment_id.clone(),
        explanation_type: new.explanation_type.clone(),
        explanation_text: new.explanation_text.clone(),
        other_type_label,
        submitted_from: constants::SUBMITTED_FROM_PC_APP.to_string(),
        // segment 메타 — 서버에 segment 가 아직 없으면 이 정보로 upsert.
        work_date: seg.work_date.format("%Y-%m-%d").to_string(),
        segment_type: seg.segment_type.as_str().to_string(),
        start_time: seg.start_time,
        end_time: seg.end_time,
        duration_seconds: seg.duration_seconds,
        applied_idle_threshold_seconds: seg.applied_idle_threshold_seconds,
        policy_scope: seg.policy_scope.clone(),
    };
    let status_slot = form.status.clone();
    state.runtime.spawn(async move {
        let maybe_session = state2.session.read().unwrap().clone();
        if maybe_session.is_none() {
            return;
        }
        match state2.api.submit_explanation(payload).await {
            Ok(()) => {
                // 성공 시 로컬 row 즉시 물리 삭제 — UI "전체 소명 내역" 은 서버 응답을
                // 진실 소스로 사용하므로 로컬 흔적 불필요. (2026-05-12 변경)
                let _ = explanations_repo::delete(&state2.db, local_id);
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some("제출 완료".to_string());
                }
            }
            Err(e) => {
                // 서버 거부 `400 INVALID_EXPLANATION_TYPE` — 회사가 사유를 변경했거나
                // fallback 코드로 제출했을 때. 사용자에게 별도 안내. (Phase 1.b)
                // 다음 user-info 폴링이 자동으로 캐시를 갱신하므로 클라가 별도 호출 불필요.
                let msg_lower = e.to_string().to_ascii_uppercase();
                let invalid_type = msg_lower.contains("INVALID_EXPLANATION_TYPE")
                    || msg_lower.contains("HTTP 400");
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some(if invalid_type {
                        "사유 목록이 변경됐을 수 있습니다. 잠시 후 새로고침해 다시 선택해 주세요.\n(로컬에는 저장됨)".to_string()
                    } else {
                        format!("서버 전송 실패 — 로컬에 저장됨: {e}")
                    });
                }
            }
        }
        // 성공·실패 모두 서버 진실 응답으로 동기화 — 옵티미스틱 마킹이 잘못된 상태라면
        // 자동 복원되고, 정상이라면 새 SUBMITTED 응답으로 안정화.
        crate::ui::explanation_list_view::request_refresh(state2.clone(), employee_id);
    });
}

/// 서버 응답(`RemoteExplanation`)을 입력 화면이 사용하는 `IdleSegment` 로 변환.
/// 다른 PC 에서 만든 segment / 로컬에 없는 SUBMITTED row 도 입력 화면에 표시·재제출 가능하게 함.
/// (2026-05-13 핫픽스)
fn remote_to_idle_segment(r: RemoteExplanation, session: &User, device_id: &str) -> IdleSegment {
    let work_date = NaiveDate::parse_from_str(&r.work_date, "%Y-%m-%d")
        .unwrap_or_else(|_| Utc::now().date_naive());
    let start_time = r.start_time.unwrap_or_else(Utc::now);
    let segment_type = SegmentType::parse(&r.segment_type).unwrap_or(SegmentType::PcIdle);
    let explanation_status = ExplanationStatus::parse(&r.explanation_status);
    IdleSegment {
        id: 0,
        segment_id: r.segment_id,
        company_id: session.company_id_str.clone(),
        employee_id: session.employee_id_str.clone(),
        device_id: device_id.to_string(),
        work_date,
        segment_type,
        start_time,
        end_time: r.end_time,
        duration_seconds: r.duration_seconds,
        applied_idle_threshold_seconds: r.applied_idle_threshold_seconds,
        policy_scope: "DEFAULT".to_string(), // 서버 응답엔 없음 — 재제출 메타용 fallback
        explanation_required: true,
        explanation_deadline: r.explanation_deadline,
        explanation_status,
    }
}
