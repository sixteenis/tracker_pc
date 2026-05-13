//! ============================================================================
//! ui::explanation_list_view — 전체 소명 내역 화면 (상태별 탭 필터).
//! ============================================================================
//!
//! 본 화면 진입 경로:
//!   - 상태 화면의 "전체 소명 내역" 버튼
//!   - 출퇴근 기록 카드 옆 "소명 N건 ▶" 진입 버튼
//!   - 트레이 메뉴 "근무시간 소명"
//!
//! ── 데이터 소스 (2026-05-12 변경, 사용자 지시) ─────────────────────
//! 본 화면은 **서버 응답을 진실 소스**로 사용한다. `GET /api/pc-agent/worktime-explanations`
//! 응답(`Vec<RemoteExplanation>`)을 화면 진입 시 1회 호출 + "새로고침" 버튼으로 재조회.
//! 만료 row 제외 등 모든 필터링은 서버가 처리하므로 클라가 다시 가공하지 않는다.
//! 로컬 `idle_segments` 테이블은 idle 감지 / 오프라인 큐 용도로만 사용한다.
//!
//! 상단 4개 카테고리 탭으로 상태별 필터:
//!   - 전체        : 모든 segment (서버 응답 모두)
//!   - 소명 필요   : PENDING       (사용자 액션 필요. EXPIRED 는 "소명 필요" 에서 제외 — 서버가 응답에서 제외하면 표시 안 됨)
//!   - 검토중      : SUBMITTED
//!   - 승인 완료   : EXEMPTED
//!
//! 표시: 날짜 / 시작 / 종료 / 간격 / 적용 기준 / 종류 / 상태 / 소명 마감 + 소명하기 버튼.
//! "소명하기" 버튼은 사용자 액션이 의미 있는 PENDING/EXPIRED 상태일 때만 노출.
//!
//! ── 캐시 ───────────────────────────────────────────────────────────
//! 모듈 내 static `CACHE` 에 마지막 응답 + 로딩/에러 상태 보관. UI 매 프레임이
//! 새 API 호출을 만들지 않도록. 다른 사용자 EMPSID 로 전환되면 캐시 무효화.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, NaiveDate, Utc};
use eframe::egui;
use once_cell::sync::Lazy;

use crate::app::AppState;
use crate::data::dto::RemoteExplanation;
use crate::data::local::idle_segments_repo::ExplanationStatus;
use crate::ui::{GRAY_TEXT, ORANGE, Route};
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplanationFilter {
    All,
    Needs,
    Review,
    Approved,
}

impl Default for ExplanationFilter {
    fn default() -> Self {
        Self::Needs
    }
}

impl ExplanationFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "전체",
            Self::Needs => "소명 필요",
            Self::Review => "검토중",
            Self::Approved => "승인 완료",
        }
    }

    fn matches(self, status: ExplanationStatus) -> bool {
        match self {
            Self::All => true,
            Self::Needs => matches!(status, ExplanationStatus::Pending),
            Self::Review => matches!(status, ExplanationStatus::Submitted),
            Self::Approved => matches!(status, ExplanationStatus::Exempted),
        }
    }
}

#[derive(Default)]
struct ListCache {
    /// 마지막 응답. `None` = 아직 한 번도 로드 안 함.
    items: Option<Vec<RemoteExplanation>>,
    /// 현재 비동기 호출 중 여부.
    loading: bool,
    /// 마지막 호출 에러. 성공 시 `None`.
    error: Option<String>,
    /// 어떤 EMPSID 의 응답인지 — 다른 사용자 전환 시 캐시 무효화.
    employee_id: i64,
    last_fetched_at: Option<DateTime<Utc>>,
}

static CACHE: Lazy<RwLock<ListCache>> = Lazy::new(|| RwLock::new(ListCache::default()));

/// 화면 진입점 — 헤더 + 탭 + 콘텐츠.
pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    route: &mut Route,
    last_toast_at: &mut Option<DateTime<Utc>>,
    filter: &mut ExplanationFilter,
) {
    crate::ui::orange_header(ctx, "전체 소명 내역", "메인", route, Route::Status);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(crate::ui::BG).inner_margin(egui::Margin::same(20.0)))
        .show(ctx, |ui| {
            content(ui, state, route, last_toast_at, filter);
        });
}

fn content(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    route: &mut Route,
    last_toast_at: &mut Option<DateTime<Utc>>,
    filter: &mut ExplanationFilter,
) {
    let session = match state.session.read().unwrap().clone() {
        Some(s) => s,
        None => {
            ui.colored_label(egui::Color32::YELLOW, "로그인이 필요합니다.");
            return;
        }
    };

    // 화면 진입 직후 한 번 또는 사용자 전환 시 캐시 무효화 + 자동 로드.
    ensure_loaded(state, session.employee_id);

    // 캐시 snapshot.
    let (items, loading, error) = {
        let g = CACHE.read().unwrap();
        (
            g.items.clone(),
            g.loading,
            g.error.clone(),
        )
    };
    let items_ref: &[RemoteExplanation] = items.as_deref().unwrap_or(&[]);

    // ── 탭 바 (상태별 카테고리) ─────────────────────────────────
    ui.horizontal(|ui| {
        for f in [
            ExplanationFilter::All,
            ExplanationFilter::Needs,
            ExplanationFilter::Review,
            ExplanationFilter::Approved,
        ] {
            let count = items_ref
                .iter()
                .filter(|s| f.matches(ExplanationStatus::parse(&s.explanation_status)))
                .count();
            let active = *filter == f;
            let label = if count > 0 {
                format!("{} {}", f.label(), count)
            } else {
                f.label().to_string()
            };
            let text_color = if active { ORANGE } else { GRAY_TEXT };
            let btn = egui::Button::new(
                egui::RichText::new(label).size(14.0).color(text_color).strong(),
            )
            .frame(false);
            let resp = ui.add(btn);
            if resp.clicked() {
                *filter = f;
            }
            if active {
                let underline_rect = egui::Rect::from_min_size(
                    resp.rect.left_bottom() + egui::vec2(0.0, 2.0),
                    egui::vec2(resp.rect.width(), 2.5),
                );
                ui.painter().rect_filled(underline_rect, egui::Rounding::ZERO, ORANGE);
            }
            ui.add_space(20.0);
        }

        // 새로고침 버튼 — 우측 정렬.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_label = if loading { "불러오는 중…" } else { "↻ 새로고침" };
            if ui
                .add_enabled(!loading, egui::Button::new(refresh_label).frame(false))
                .clicked()
            {
                trigger_fetch(state.clone(), session.employee_id);
            }
        });
    });

    ui.add_space(2.0);
    let sep_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        egui::vec2(ui.available_width(), 1.0),
    );
    ui.painter().rect_filled(sep_rect, egui::Rounding::ZERO, egui::Color32::from_rgb(230, 230, 230));
    ui.add_space(8.0);

    // ── 상태 메시지: 로딩 / 에러 ───────────────────────────────
    if loading && items.is_none() {
        ui.label(
            egui::RichText::new("서버에서 소명 내역을 불러오는 중…")
                .size(12.0)
                .color(GRAY_TEXT),
        );
        return;
    }
    if let Some(msg) = &error {
        ui.colored_label(
            egui::Color32::from_rgb(210, 50, 50),
            format!("불러오기 실패: {msg}"),
        );
        ui.label(
            egui::RichText::new("네트워크 상태를 확인한 뒤 위 '새로고침' 을 눌러주세요.")
                .size(12.0)
                .color(GRAY_TEXT),
        );
        ui.add_space(8.0);
        // 캐시된 이전 데이터가 있으면 그대로 표시.
    }

    let filtered: Vec<&RemoteExplanation> = items_ref
        .iter()
        .filter(|s| filter.matches(ExplanationStatus::parse(&s.explanation_status)))
        .collect();

    // 안내 문구 — 현재 탭 맥락에 맞게.
    let hint = match *filter {
        ExplanationFilter::All => "전체 자리비움 구간 내역입니다.",
        ExplanationFilter::Needs => {
            "근무시간 중 PC 미사용 시간이 감지되었습니다. 업무상 사유가 있는 경우 소명해 주세요."
        }
        ExplanationFilter::Review => "제출한 소명이 관리자 검토를 기다리고 있습니다.",
        ExplanationFilter::Approved => "사유가 인정되어 승인 완료된 자리비움입니다.",
    };
    ui.label(egui::RichText::new(hint).size(12.0).color(GRAY_TEXT));
    ui.add_space(6.0);

    // 사용자가 이미 목록 화면을 보고 있으므로 토스트는 띄우지 않는다.
    let _ = last_toast_at;

    if filtered.is_empty() {
        ui.add_space(20.0);
        let empty_msg = match *filter {
            ExplanationFilter::All => "표시할 자리비움 구간이 없습니다.",
            ExplanationFilter::Needs => "소명이 필요한 자리비움 구간이 없습니다.",
            ExplanationFilter::Review => "검토 대기 중인 소명이 없습니다.",
            ExplanationFilter::Approved => "승인 완료된 소명이 없습니다.",
        };
        ui.colored_label(egui::Color32::GRAY, empty_msg);

        if matches!(*filter, ExplanationFilter::Needs | ExplanationFilter::All) {
            ui.label(
                "자리비움 구간은 PC 미사용 시간이 회사 정책의 기준 시간을 초과하면 자동으로 생성됩니다.\n\
                 현재 회사 정책 기준 시간은 상태 화면에서 확인할 수 있습니다.",
            );
        }
        return;
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

            for seg in &filtered {
                let work_date = NaiveDate::parse_from_str(&seg.work_date, "%Y-%m-%d")
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|_| seg.work_date.clone());
                ui.label(work_date);
                ui.label(
                    seg.start_time
                        .as_ref()
                        .map(util::format_local_time)
                        .unwrap_or_else(|| "—".into()),
                );
                ui.label(
                    seg.end_time
                        .as_ref()
                        .map(util::format_local_time)
                        .unwrap_or_else(|| "진행 중".into()),
                );
                ui.label(
                    seg.duration_seconds
                        .map(util::format_duration_human)
                        .unwrap_or_else(|| "진행 중".into()),
                );
                ui.label(format!(
                    "{} 이내",
                    util::format_duration_human(seg.applied_idle_threshold_seconds)
                ));
                ui.label(&seg.segment_type);
                let status = ExplanationStatus::parse(&seg.explanation_status);
                ui.label(status_label(status));
                ui.label(
                    seg.explanation_deadline
                        .as_ref()
                        .map(util::format_local_dt)
                        .unwrap_or_else(|| "—".into()),
                );
                // 소명하기 버튼 — 사용자 액션 가능한 상태(PENDING/EXPIRED)에서만.
                // 서버가 만료 row 를 응답에서 제외하므로 보통은 PENDING 만 보이지만,
                // 클라가 늦게 새로고침하기 전이라면 EXPIRED 가 표시될 수도 있어 둘 다 허용.
                let actionable = matches!(
                    status,
                    ExplanationStatus::Pending | ExplanationStatus::Expired
                );
                if actionable {
                    if ui.button("소명하기").clicked() {
                        *route = Route::ExplanationInput { segment_id: seg.segment_id.clone() };
                    }
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
    });
}

fn status_label(status: ExplanationStatus) -> &'static str {
    match status {
        ExplanationStatus::Pending => "소명 필요",
        ExplanationStatus::Submitted => "검토중",
        ExplanationStatus::Expired => "기한 만료",
        ExplanationStatus::Exempted => "승인 완료",
    }
}

/// 화면 진입 시 자동 로드 — 캐시 비어있거나 다른 EMPSID 면 fetch trigger.
fn ensure_loaded(state: &Arc<AppState>, employee_id: i64) {
    let need_fetch = {
        let g = CACHE.read().unwrap();
        let user_changed = g.items.is_some() && g.employee_id != employee_id;
        let never_loaded = g.items.is_none() && g.error.is_none() && !g.loading;
        if user_changed {
            true
        } else {
            never_loaded
        }
    };
    if need_fetch {
        trigger_fetch(state.clone(), employee_id);
    }
}

/// 새 비동기 fetch 시작. 중복 호출 방지를 위해 loading 플래그 set.
fn trigger_fetch(state: Arc<AppState>, employee_id: i64) {
    {
        let mut g = CACHE.write().unwrap();
        if g.loading {
            return;
        }
        g.loading = true;
        // 사용자가 바뀌면 옛 items 보여주면 안 됨 — 비움.
        if g.employee_id != employee_id {
            g.items = None;
        }
        g.employee_id = employee_id;
        g.error = None;
    }
    state.runtime.clone().spawn(async move {
        let result = state.api.list_explanations(employee_id).await;
        let mut g = CACHE.write().unwrap();
        g.loading = false;
        g.last_fetched_at = Some(Utc::now());
        match result {
            Ok(items) => {
                g.items = Some(items);
                g.error = None;
            }
            Err(e) => {
                g.error = Some(e.to_string());
            }
        }
    });
}

/// 로그아웃 시 호출 — 다른 계정 데이터 노출 방지.
pub fn clear_cache() {
    let mut g = CACHE.write().unwrap();
    *g = ListCache::default();
}

/// 제출 직후 옵티미스틱 업데이트 — 캐시의 해당 segment 상태를 SUBMITTED 로 즉시 전환.
/// "소명 필요" 탭에서 바로 사라지고 "검토중" 으로 이동한 것처럼 보이게 한다.
/// 이후 `request_refresh` 가 서버 진실 응답으로 동기화 (서버 거부 시 자동 복원).
pub fn mark_submitted_optimistic(segment_id: &str) {
    let mut g = CACHE.write().unwrap();
    let Some(items) = g.items.as_mut() else { return };
    for it in items.iter_mut() {
        if it.segment_id == segment_id {
            it.explanation_status = ExplanationStatus::Submitted.as_str().to_string();
        }
    }
}

/// 외부에서 강제 재조회 트리거 — 제출 성공/실패 직후 서버 진실 동기화.
pub fn request_refresh(state: Arc<AppState>, employee_id: i64) {
    trigger_fetch(state, employee_id);
}
