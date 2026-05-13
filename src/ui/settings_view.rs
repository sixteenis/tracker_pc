// ============================================================================
// ui::settings_view — 환경설정 화면 (3 탭 + dev 전용 1 탭).
// ============================================================================
//
// 탭 구성:
//   - 일반        : 자동 실행 / 알림 / 트레이 토글 (현재는 메모리 only)
//   - 감지 설정   : 정책 정보 read-only (운영자/근로자용 요약)
//   - 계정        : 사번/팀/기기/버전 + 로그아웃 버튼
//   - 회사설정    : `GET /api/pc-agent/policy` 응답 원본 dump + **편집/저장(PATCH)**
//                  → `#[cfg(debug_assertions)]` 로 release 빌드에서 자동 제외.
//                    출시 빌드(`cargo build --release`) 에는 탭 자체가 컴파일되지
//                    않으므로 사용자에게 노출되지 않는다.
//                    편집 폼은 회사 관리자(`Emply.Author>=5`) 한정 — 권한 없으면
//                    서버가 403 반환, 클라는 사용자 친화 메시지로 표시.
//
// TODO(미구현): 토글 3개 (auto_start / notifications / tray_icon) 가 화면에서만
// 동작. 실제 OS 설정 적용 안 됨.
//   - auto_start  : Windows 의 HKCU\...\Run 레지스트리 또는 macOS LaunchAgent
//   - notifications: notify::show_explanation_request 호출 전 토글 체크
//   - tray_icon   : tray-icon 의존성 통합 (1차에서는 비활성)
// TODO(미구현): "프로그램 종료 허용" 정책이 표시만 되고 실제 강제 적용 안 됨.
// 관리자가 끄면 사용자가 종료 못 하게 하려면 Windows shell hook 또는 별도 watchdog 필요.
// TODO(영속화): 토글 상태를 `db::settings_repo` 에 저장해서 재시작 후에도 유지.

use std::sync::Arc;
#[cfg(debug_assertions)]
use std::sync::Mutex;

use eframe::egui;

use crate::app::AppState;
use crate::ui::{GRAY_TEXT, NAVY, ORANGE, Route};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Detection,
    Account,
    /// dev 전용 — release 빌드 시 컴파일에서 제외됨.
    #[cfg(debug_assertions)]
    CompanyPolicy,
    /// dev 전용 — 회사 커스텀 소명사유 CMS CRUD 테스트 (Phase 2, 2026-05-12).
    #[cfg(debug_assertions)]
    CompanyExplanation,
}

pub struct SettingsUi {
    pub auto_start: bool,
    pub notifications: bool,
    pub tray_icon: bool,
    /// 회사 정책 편집 폼 — dev 전용. release 빌드 시 제외.
    #[cfg(debug_assertions)]
    pub policy_edit: PolicyEditForm,
    /// 회사 소명사유 CMS 테스트 폼 — dev 전용.
    #[cfg(debug_assertions)]
    pub explanation_cms: ExplanationCmsForm,
}

impl Default for SettingsUi {
    fn default() -> Self {
        Self {
            auto_start: true,
            notifications: true,
            tray_icon: true,
            #[cfg(debug_assertions)]
            policy_edit: PolicyEditForm::default(),
            #[cfg(debug_assertions)]
            explanation_cms: ExplanationCmsForm::default(),
        }
    }
}

/// 회사 소명사유 CMS CRUD 테스트 폼 — dev 전용. release 빌드 시 컴파일 제외.
///
/// 테스트 화면: 현재 활성 사유 목록 표시 + 4개 CRUD 카드(추가/수정/비활성/사용통계).
/// 운영 화면이 아니라 단순 endpoint 호출 + 응답 표시 패턴 (사용자 결정 2026-05-12).
#[cfg(debug_assertions)]
pub struct ExplanationCmsForm {
    // 추가(POST) — 2026-05-12: `code` 는 서버 자동 생성, 클라 입력 X.
    pub create_label: String,
    pub create_sort: i32,
    pub create_icon: String,
    pub create_requires_text: bool,

    // 수정(PATCH /:sid)
    pub patch_sid: i64,
    pub patch_label: String,
    pub patch_sort: i32,
    pub patch_icon: String,
    pub patch_requires_text: bool,

    // 비활성(PATCH /:sid/deactivate)
    pub deactivate_sid: i64,

    // 사용 통계(GET usage)
    pub usage_days: u32,

    pub status: Arc<Mutex<Option<String>>>,
    pub pending: Arc<Mutex<bool>>,
}

#[cfg(debug_assertions)]
impl Default for ExplanationCmsForm {
    fn default() -> Self {
        Self {
            create_label: String::new(),
            create_sort: 1000,
            create_icon: String::new(),
            create_requires_text: false,
            patch_sid: 0,
            patch_label: String::new(),
            patch_sort: 0,
            patch_icon: String::new(),
            patch_requires_text: false,
            deactivate_sid: 0,
            usage_days: 30,
            status: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(false)),
        }
    }
}

/// 회사 정책 편집 폼 — dev 전용. release 빌드 시 컴파일에서 제외.
///
/// 사용 패턴:
///   - 탭 진입 시 `initialized==false` 면 `state.snapshot_policy()` 로 폼 + `original_*` 채움.
///   - 사용자가 필드 수정.
///   - "저장" 클릭 → 변경된 필드만 `PolicyPatchFields` 에 담아 `patch_policy` 호출.
///   - 응답 OK → `state.policy` / `state.status` 갱신 + `original_*` 새 값 반영.
///   - 응답 Err → `status` 슬롯에 사용자 친화 메시지.
#[cfg(debug_assertions)]
pub struct PolicyEditForm {
    pub idle_threshold_seconds: u64,
    pub lunch_start_time: String,
    pub lunch_end_time: String,
    pub lunch_allowed_minutes: u32,
    pub explanation_deadline_hours: u32,
    pub can_track_time: bool,
    pub reason: String,

    // 비교용 원본 — 저장 시 변경된 필드만 patch 에 포함.
    original_idle: u64,
    original_lunch_start: String,
    original_lunch_end: String,
    original_lunch_mins: u32,
    original_expl_hours: u32,
    original_can_track: bool,
    original_policy_version: i64,

    /// 폼 초기화 여부. false 면 다음 프레임에 `state.snapshot_policy()` 로 채움.
    initialized: bool,
    pub status: Arc<Mutex<Option<String>>>,
    pub pending: Arc<Mutex<bool>>,
}

#[cfg(debug_assertions)]
impl Default for PolicyEditForm {
    fn default() -> Self {
        Self {
            idle_threshold_seconds: 0,
            lunch_start_time: String::new(),
            lunch_end_time: String::new(),
            lunch_allowed_minutes: 0,
            explanation_deadline_hours: 0,
            can_track_time: false,
            reason: String::new(),
            original_idle: 0,
            original_lunch_start: String::new(),
            original_lunch_end: String::new(),
            original_lunch_mins: 0,
            original_expl_hours: 0,
            original_can_track: false,
            original_policy_version: 0,
            initialized: false,
            status: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(false)),
        }
    }
}

#[cfg(debug_assertions)]
impl PolicyEditForm {
    /// `state.snapshot_policy()` 의 현재 값으로 폼 + 원본 채우기.
    fn init_from(&mut self, p: &crate::data::dto::PolicySnapshot) {
        self.idle_threshold_seconds = p.effective_idle_threshold_seconds;
        self.lunch_start_time = p.lunch_start_time.clone();
        self.lunch_end_time = p.lunch_end_time.clone();
        self.lunch_allowed_minutes = p.lunch_allowed_minutes;
        self.explanation_deadline_hours = p.explanation_deadline_hours;
        self.can_track_time = p.can_track_time;
        self.original_idle = self.idle_threshold_seconds;
        self.original_lunch_start = self.lunch_start_time.clone();
        self.original_lunch_end = self.lunch_end_time.clone();
        self.original_lunch_mins = self.lunch_allowed_minutes;
        self.original_expl_hours = self.explanation_deadline_hours;
        self.original_can_track = self.can_track_time;
        self.original_policy_version = p.policy_version;
        self.initialized = true;
    }

    /// 현재 값 vs 원본 비교해서 변경된 필드만 채운 patch 반환.
    fn diff_patch(&self) -> crate::data::dto::PolicyPatchFields {
        let mut p = crate::data::dto::PolicyPatchFields::default();
        if self.idle_threshold_seconds != self.original_idle {
            p.idle_threshold_seconds = Some(self.idle_threshold_seconds);
        }
        if self.lunch_start_time != self.original_lunch_start {
            p.lunch_start_time = Some(self.lunch_start_time.clone());
        }
        if self.lunch_end_time != self.original_lunch_end {
            p.lunch_end_time = Some(self.lunch_end_time.clone());
        }
        if self.lunch_allowed_minutes != self.original_lunch_mins {
            p.lunch_allowed_minutes = Some(self.lunch_allowed_minutes);
        }
        if self.explanation_deadline_hours != self.original_expl_hours {
            p.explanation_deadline_hours = Some(self.explanation_deadline_hours);
        }
        if self.can_track_time != self.original_can_track {
            p.can_track_time = Some(self.can_track_time);
        }
        p
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
    // dev 전용 "회사설정" 탭은 `#[cfg(debug_assertions)]` 로만 포함 — release 빌드에서 자동 제외.
    // 회사설정 / 사유 관리 탭은 Author ∈ {1, 2} (최고/총괄 관리자) 만 노출 (2026-05-12 결정).
    // dev 빌드에서도 권한 없는 사용자는 메뉴 안 보임. release 빌드는 cfg 가드로 어차피 제외.
    #[allow(unused_mut)]
    let mut tabs: Vec<(&str, SettingsTab)> = vec![
        ("일반", SettingsTab::General),
        ("감지 설정", SettingsTab::Detection),
        ("계정", SettingsTab::Account),
    ];
    #[cfg(debug_assertions)]
    if is_company_admin(state) {
        tabs.push(("회사설정", SettingsTab::CompanyPolicy));
        tabs.push(("사유 관리(테스트)", SettingsTab::CompanyExplanation));
    }
    // Author 권한이 사라진 케이스(현재 회사설정 탭에 있는데 다른 계정 전환 등) — 안전한 기본 탭으로.
    if !tabs.iter().any(|(_, t)| *t == *tab) {
        *tab = SettingsTab::General;
    }
    ui.horizontal(|ui| {
        ui.add_space(pad);
        for (label, t) in tabs.iter().copied() {
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
            #[cfg(debug_assertions)]
            SettingsTab::CompanyPolicy => {
                company_policy_tab(ui, pad, state, &mut settings.policy_edit);
            }
            #[cfg(debug_assertions)]
            SettingsTab::CompanyExplanation => {
                company_explanation_tab(ui, pad, state, &mut settings.explanation_cms);
            }
            SettingsTab::Account => {
                // 사용자/회사/팀 정보는 도메인 서비스에서 직접 가져온다.
                let user = crate::domain::service::user_service::current();
                let company = crate::domain::service::company_service::current();
                let team = crate::domain::service::team_service::current();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        if let Some(u) = &user {
                            info_row(ui, "이름", &u.name_for_display());
                            info_row(ui, "이메일", &u.email);
                            info_row(ui, "사원 ID", &u.employee_id.to_string());
                            info_row(
                                ui,
                                "회사",
                                &company.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| "—".to_string()),
                            );
                            info_row(
                                ui,
                                "팀",
                                &team.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "—".to_string()),
                            );
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
                            let _ = crate::domain::service::user_service::logout(
                                state,
                                crate::domain::service::user_service::LogoutReason::UserAction,
                            );
                            crate::ui::explanation_list_view::clear_cache();
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

// ── 회사설정 탭 (dev 전용) ────────────────────────────────────────────
//
// `GET /policy` 응답 원본 dump + 편집 폼 + `PATCH /policy` 저장 버튼.
// 회사 관리자(`Emply.Author>=5`) 만 저장 가능 — 권한 없으면 서버가 403,
// 클라는 응답 메시지에 따라 안내. release 빌드에서는 본 함수 컴파일 제외.

#[cfg(debug_assertions)]
fn company_policy_tab(
    ui: &mut egui::Ui,
    pad: f32,
    state: &Arc<AppState>,
    form: &mut PolicyEditForm,
) {
    let p = state.snapshot_policy();
    let live = state.snapshot_status();

    // 첫 진입 또는 외부에서 정책이 갱신된 경우(version 변화) 폼을 새 값으로 초기화.
    if !form.initialized || form.original_policy_version != p.policy_version {
        form.init_from(&p);
    }

    let opt = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "—".to_string());

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("⚙ /api/pc-agent/policy (dev — 회사 관리자 전용)")
                    .size(13.0)
                    .color(ORANGE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "출시 빌드에는 포함되지 않음. 저장 시 서버 PATCH 호출 (회사 관리자 권한 필요).",
                )
                .size(11.0)
                .color(GRAY_TEXT),
            );

            // ── 편집 폼 ─────────────────────────────────────────────
            ui.add_space(12.0);
            ui.label(egui::RichText::new("편집").size(12.0).color(NAVY).strong());
            ui.add_space(4.0);

            // 라벨에는 한글이름 + (영문식별자) 만 표시, 단위는 입력 필드 우측 고정 suffix.
            // 범위 검증 폐기 (2026-05-12 사용자 결정) — 사용자가 입력한 값 그대로 송신,
            // 서버가 단독 검증해 400 응답 시 친화 메시지로 안내.
            egui::Grid::new("policy_edit_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("자리비움 임계치 (idle_threshold_seconds)");
                    ui.add(egui::DragValue::new(&mut form.idle_threshold_seconds).suffix(" 초"));
                    ui.end_row();

                    ui.label("점심 시작 시각 (lunch_start_time)");
                    ui.text_edit_singleline(&mut form.lunch_start_time);
                    ui.end_row();

                    ui.label("점심 종료 시각 (lunch_end_time)");
                    ui.text_edit_singleline(&mut form.lunch_end_time);
                    ui.end_row();

                    ui.label("점심 인정 시간 (lunch_allowed_minutes)");
                    ui.add(egui::DragValue::new(&mut form.lunch_allowed_minutes).suffix(" 분"));
                    ui.end_row();

                    ui.label("소명 마감 시간 (explanation_deadline_hours)");
                    ui.add(
                        egui::DragValue::new(&mut form.explanation_deadline_hours).suffix(" 시간"),
                    );
                    ui.end_row();

                    ui.label("PC 추적 허용 (can_track_time)");
                    ui.checkbox(&mut form.can_track_time, "추적 허용");
                    ui.end_row();

                    ui.label("변경 사유 메모 (reason)");
                    ui.text_edit_singleline(&mut form.reason);
                    ui.end_row();
                });

            ui.add_space(8.0);
            let patch = form.diff_patch();
            let is_dirty = !patch.is_empty();
            let is_pending = form.pending.lock().map(|g| *g).unwrap_or(false);

            ui.horizontal(|ui| {
                let save_btn = ui.add_enabled(
                    is_dirty && !is_pending,
                    egui::Button::new(if is_pending {
                        "저장 중…"
                    } else {
                        "저장 (PATCH)"
                    }),
                );
                if save_btn.clicked() {
                    submit_policy_patch(state, form);
                }
                if ui
                    .add_enabled(is_dirty, egui::Button::new("초기값으로"))
                    .clicked()
                {
                    form.init_from(&p);
                    if let Ok(mut s) = form.status.lock() {
                        *s = Some("입력 초기화됨".to_string());
                    }
                }
                ui.label(
                    egui::RichText::new(if is_dirty {
                        "변경 사항 있음"
                    } else {
                        "변경 사항 없음"
                    })
                    .size(11.0)
                    .color(if is_dirty { ORANGE } else { GRAY_TEXT }),
                );
            });

            if let Ok(s) = form.status.lock() {
                if let Some(msg) = s.as_ref() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(msg).size(12.0).color(NAVY));
                }
            }

            // ── 응답 원본 / 라이브 상태 ────────────────────────────
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new("─ /api/pc-agent/policy 응답 원본 ─")
                    .size(12.0)
                    .color(GRAY_TEXT),
            );
            ui.add_space(4.0);
            info_row(ui, "정책 버전 (policy_version)", &p.policy_version.to_string());
            info_row(ui, "정책 스코프 (policy_scope)", &p.policy_scope);
            info_row(
                ui,
                "적용 자리비움 임계치 — 초 (effective_idle_threshold_seconds)",
                &p.effective_idle_threshold_seconds.to_string(),
            );
            info_row(
                ui,
                "회사 임계치 — 초 (company_idle_threshold_seconds)",
                &opt(p.company_idle_threshold_seconds),
            );
            info_row(
                ui,
                "팀 임계치 — 초 (team_idle_threshold_seconds)",
                &opt(p.team_idle_threshold_seconds),
            );
            info_row(
                ui,
                "근로자 임계치 — 초 (employee_idle_threshold_seconds)",
                &opt(p.employee_idle_threshold_seconds),
            );
            info_row(ui, "점심 시작 (lunch_start_time)", &p.lunch_start_time);
            info_row(ui, "점심 종료 (lunch_end_time)", &p.lunch_end_time);
            info_row(
                ui,
                "점심 인정 — 분 (lunch_allowed_minutes)",
                &p.lunch_allowed_minutes.to_string(),
            );
            info_row(
                ui,
                "소명 마감 — 시간 (explanation_deadline_hours)",
                &p.explanation_deadline_hours.to_string(),
            );
            info_row(ui, "PC 추적 허용 (can_track_time)", &p.can_track_time.to_string());

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("─ 라이브 상태 (LiveStatus.*) ─")
                    .size(12.0)
                    .color(GRAY_TEXT),
            );
            ui.add_space(4.0);
            info_row(
                ui,
                "라이브 정책 버전 (status.policy_version)",
                &live.policy_version.to_string(),
            );
            info_row(ui, "라이브 정책 스코프 (status.policy_scope)", &live.policy_scope);
            info_row(
                ui,
                "라이브 적용 임계치 — 초 (status.effective_idle_threshold_seconds)",
                &live.effective_idle_threshold_seconds.to_string(),
            );
            info_row(
                ui,
                "라이브 추적 허용 (status.can_track_time)",
                &live.can_track_time.to_string(),
            );
            info_row(
                ui,
                "마지막 정책 동기화 (status.last_policy_sync_at)",
                &live
                    .last_policy_sync_at
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "—".to_string()),
            );
        });
    });
}

/// "저장 (PATCH)" 클릭 시 호출. 폼 1차 유효성 → spawn → 응답 처리.
#[cfg(debug_assertions)]
fn submit_policy_patch(state: &Arc<AppState>, form: &mut PolicyEditForm) {
    let patch = form.diff_patch();
    if patch.is_empty() {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("변경 사항이 없습니다.".to_string());
        }
        return;
    }
    if let Err(msg) = patch.validate() {
        if let Ok(mut s) = form.status.lock() {
            *s = Some(format!("입력 오류: {msg}"));
        }
        return;
    }
    let session = match state.session.read().unwrap().clone() {
        Some(s) => s,
        None => {
            if let Ok(mut s) = form.status.lock() {
                *s = Some("로그인 후 저장 가능합니다.".to_string());
            }
            return;
        }
    };
    let req = crate::data::dto::PolicyPatchRequest {
        requester_emp_sid: session.employee_id,
        patch,
        reason: if form.reason.trim().is_empty() {
            None
        } else {
            Some(form.reason.trim().to_string())
        },
    };

    // pending 플래그 → 버튼 비활성화 + UI 표시.
    if let Ok(mut p) = form.pending.lock() {
        *p = true;
    }
    if let Ok(mut s) = form.status.lock() {
        *s = Some("저장 중…".to_string());
    }

    let state2 = state.clone();
    let status_slot = form.status.clone();
    let pending_slot = form.pending.clone();
    state.runtime.spawn(async move {
        let outcome = state2.api.patch_policy(req).await;
        // pending 해제는 결과 무관 항상.
        if let Ok(mut p) = pending_slot.lock() {
            *p = false;
        }
        match outcome {
            Ok(snapshot) => {
                // state.policy 갱신 — UI 가 다음 프레임에 새 값을 초기화 (version 비교).
                if let Ok(mut w) = state2.policy.write() {
                    *w = snapshot.clone();
                }
                // LiveStatus 의 정책 derive 필드도 즉시 반영 (policy_sync 30분 폴링 기다리지 않게).
                if let Ok(mut s) = state2.status.write() {
                    s.effective_idle_threshold_seconds = snapshot.effective_idle_threshold_seconds;
                    s.policy_scope = snapshot.policy_scope.clone();
                    s.policy_version = snapshot.policy_version;
                }
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some(format!(
                        "저장 완료 (policy_version → {})",
                        snapshot.policy_version
                    ));
                }
            }
            Err(e) => {
                // 서버 응답 본문에 따라 사용자 친화 메시지 매핑.
                let msg = e.to_string();
                let upper = msg.to_ascii_uppercase();
                let pretty = if upper.contains("FORBIDDEN") || upper.contains("HTTP 403") {
                    "권한이 없습니다 — 회사 관리자(Emply.Author≥5)만 저장 가능합니다.".to_string()
                } else if upper.contains("INVALID_PATCH") || upper.contains("INVALID_FIELD") {
                    format!("입력이 서버 검증을 통과하지 못했습니다: {msg}")
                } else if upper.contains("HTTP 404") {
                    "요청자 EMPSID 가 서버에 없습니다 (재로그인 필요).".to_string()
                } else {
                    format!("저장 실패: {msg}")
                };
                if let Ok(mut s) = status_slot.lock() {
                    *s = Some(pretty);
                }
            }
        }
    });
}

// ── 회사 소명사유 CMS CRUD 테스트 탭 (dev 전용) ───────────────────────
//
// 회사 관리자(`Emply.Author>=5`) 가 사용할 4개 CMS endpoint 를 테스트 호출하는
// 단순 화면. 운영 화면이 아니라 단순 입력 폼 + 결과 표시 패턴.
// `#[cfg(debug_assertions)]` 로 release 빌드에서 자동 제외.

#[cfg(debug_assertions)]
fn company_explanation_tab(
    ui: &mut egui::Ui,
    pad: f32,
    state: &Arc<AppState>,
    form: &mut ExplanationCmsForm,
) {
    let is_pending = form.pending.lock().map(|g| *g).unwrap_or(false);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("⚙ 회사 소명사유 CMS (dev — 회사 관리자 전용)")
                    .size(13.0)
                    .color(ORANGE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "출시 빌드에는 포함되지 않음. 4개 endpoint 테스트 호출 — 회사 관리자(Author ∈ {1, 2})만 통과.",
                )
                .size(11.0)
                .color(GRAY_TEXT),
            );

            // ── 현재 활성 사유 (worker GET 캐시 — explanation_type_service) ──
            ui.add_space(12.0);
            ui.label(egui::RichText::new("현재 활성 사유 (캐시)").size(12.0).color(NAVY).strong());
            ui.add_space(4.0);
            let types = crate::domain::service::explanation_type_service::current_types();
            for t in &types {
                let req = if t.requires_text { " · 텍스트필수" } else { "" };
                let sys = if t.is_system { " · 시드" } else { "" };
                let prot = if t.is_protected { " · 보호(비활성 불가)" } else { "" };
                let sid_str = t
                    .exptype_sid
                    .map(|n| format!("sid={n}, "))
                    .unwrap_or_default();
                info_row(
                    ui,
                    &format!("{} ({})", t.label, t.code),
                    &format!("{sid_str}sort={}{req}{sys}{prot}", t.sort_order),
                );
            }
            ui.label(
                egui::RichText::new(
                    "↑ 각 row 의 sid 를 비활성 카드에 입력해 비활성화/수정 가능. \
                     (2026-05-12 부터 워커 GET 응답에도 sid 포함)",
                )
                .size(10.0)
                .color(GRAY_TEXT),
            );

            // ── 추가 (POST) ──────────────────────────────────────────
            ui.add_space(14.0);
            ui.label(egui::RichText::new("추가 (POST)").size(12.0).color(NAVY).strong());
            // 2026-05-12 시그니처 변경: code 는 서버 자동 생성 — 클라 입력 X.
            // 응답 메시지에 자동 생성된 code 노출.
            ui.label(
                egui::RichText::new(
                    "↑ code 는 서버 자동 생성(예: CUSTOM_A3F02D8C77). 라벨은 한글 자유.",
                )
                .size(10.0)
                .color(GRAY_TEXT),
            );
            egui::Grid::new("cms_create_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("라벨 (label, 한글 가능)");
                ui.text_edit_singleline(&mut form.create_label);
                ui.end_row();
                ui.label("정렬 (sort_order)");
                ui.add(egui::DragValue::new(&mut form.create_sort));
                ui.end_row();
                ui.label("아이콘 (icon, 비우면 null)");
                ui.text_edit_singleline(&mut form.create_icon);
                ui.end_row();
                ui.label("텍스트 필수 (requires_text)");
                ui.checkbox(&mut form.create_requires_text, "필수");
                ui.end_row();
            });
            if ui
                .add_enabled(!is_pending, egui::Button::new("추가 호출"))
                .clicked()
            {
                submit_explanation_create(state, form);
            }

            // ── 수정 (PATCH /:sid) ──────────────────────────────────
            ui.add_space(14.0);
            ui.label(egui::RichText::new("수정 (PATCH /:sid)").size(12.0).color(NAVY).strong());
            egui::Grid::new("cms_patch_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("SID");
                ui.add(egui::DragValue::new(&mut form.patch_sid));
                ui.end_row();
                ui.label("라벨 (비우면 변경 X)");
                ui.text_edit_singleline(&mut form.patch_label);
                ui.end_row();
                ui.label("정렬 (0 이면 변경 X)");
                ui.add(egui::DragValue::new(&mut form.patch_sort));
                ui.end_row();
                ui.label("아이콘 (비우면 변경 X)");
                ui.text_edit_singleline(&mut form.patch_icon);
                ui.end_row();
                ui.label("텍스트 필수 (체크 시 강제 적용)");
                ui.checkbox(&mut form.patch_requires_text, "적용");
                ui.end_row();
            });
            if ui
                .add_enabled(!is_pending && form.patch_sid > 0, egui::Button::new("수정 호출"))
                .clicked()
            {
                submit_explanation_patch(state, form);
            }

            // ── 비활성 (PATCH /:sid/deactivate) ──────────────────────
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("비활성 (PATCH /:sid/deactivate)")
                    .size(12.0)
                    .color(NAVY)
                    .strong(),
            );
            egui::Grid::new("cms_deactivate_grid").num_columns(2).spacing([12.0, 6.0]).show(
                ui,
                |ui| {
                    ui.label("SID");
                    ui.add(egui::DragValue::new(&mut form.deactivate_sid));
                    ui.end_row();
                },
            );
            if ui
                .add_enabled(
                    !is_pending && form.deactivate_sid > 0,
                    egui::Button::new("비활성 호출"),
                )
                .clicked()
            {
                submit_explanation_deactivate(state, form);
            }

            // ── 사용 통계 (GET usage) ────────────────────────────────
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("사용 통계 (GET usage?days=)")
                    .size(12.0)
                    .color(NAVY)
                    .strong(),
            );
            egui::Grid::new("cms_usage_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("일 수 (days)");
                ui.add(egui::DragValue::new(&mut form.usage_days));
                ui.end_row();
            });
            if ui
                .add_enabled(!is_pending && form.usage_days > 0, egui::Button::new("통계 조회"))
                .clicked()
            {
                submit_explanation_usage(state, form);
            }

            // ── 응답 결과 ────────────────────────────────────────────
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(egui::RichText::new("응답 결과").size(12.0).color(NAVY).strong());
            ui.add_space(4.0);
            if let Ok(s) = form.status.lock() {
                if let Some(msg) = s.as_ref() {
                    ui.label(egui::RichText::new(msg).size(12.0).color(NAVY));
                } else {
                    ui.label(
                        egui::RichText::new("(아직 호출 안 함)").size(11.0).color(GRAY_TEXT),
                    );
                }
            }
        });
    });
}

#[cfg(debug_assertions)]
fn cms_requester_emp_sid(state: &Arc<AppState>) -> Option<i64> {
    state.session.read().ok().and_then(|g| g.as_ref().map(|s| s.employee_id))
}

/// CMS 호출 시 필요한 (`requester_emp_sid`, `cmpsid`) 쌍 — 2026-05-12 시그니처 변경.
#[cfg(debug_assertions)]
fn cms_session_keys(state: &Arc<AppState>) -> Option<(i64, i64)> {
    state
        .session
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|s| (s.employee_id, s.company_id)))
}

/// 회사설정 / 사유 관리 탭 진입 게이트 — `Emply.Author ∈ {1, 2}` 만 통과.
/// 1=최고관리자, 2=총괄관리자. 3~5(상위팀/팀/직원) 는 메뉴 미노출.
///
/// 본 함수는 dev 빌드에서만 호출 (release 는 cfg 가드로 탭 자체 컴파일 제외).
/// release 노출 정책은 사용자 결정 후 cfg 가드 제거 시 활성화.
#[cfg(debug_assertions)]
fn is_company_admin(state: &Arc<AppState>) -> bool {
    let author = state
        .session
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.authority))
        .unwrap_or(99);
    matches!(author, 1 | 2)
}

/// 4xx 서버 응답 메시지를 사용자 친화 한국어로 변환.
/// 매핑 외 에러는 원문 그대로 (디버깅 도움).
#[cfg(debug_assertions)]
fn humanize_cms_error(e: &anyhow::Error) -> String {
    let raw = e.to_string();
    let upper = raw.to_ascii_uppercase();
    if upper.contains("FORBIDDEN") || upper.contains("HTTP 403") {
        "권한이 없습니다 — 최고관리자(Author=1) 또는 총괄관리자(Author=2) 만 가능합니다.".into()
    } else if upper.contains("AT_LEAST_ONE_REQUIRED") {
        "마지막 활성 사유는 비활성화할 수 없습니다 (최소 1개 유지)".into()
    } else if upper.contains("PROTECTED_TYPE") {
        "시스템 보호 사유(예: 기타)는 비활성화할 수 없습니다.".into()
    } else if upper.contains("DUPLICATE_CODE") {
        "이미 같은 코드가 존재합니다.".into()
    } else if upper.contains("HTTP 404") {
        "대상 SID 를 찾을 수 없습니다 (이미 비활성 또는 다른 회사 row 일 수 있음).".into()
    } else if upper.contains("INVALID_FIELD") || upper.contains("HTTP 400") {
        format!("입력 검증 실패: {raw}")
    } else {
        raw
    }
}

/// CMS 변경(추가/수정/비활성) 직후 클라 캐시 즉시 갱신 — `user_info_sync` 폴링
/// 주기(1h/5min) 기다리지 않고 `list_explanation_types` 호출 → `store_response`.
/// 소명하기 화면의 콤보가 다음 프레임부터 새 사유를 표시한다.
#[cfg(debug_assertions)]
async fn refresh_explanation_cache(state: &Arc<AppState>) {
    let session = state.session.read().ok().and_then(|g| g.clone());
    let Some(session) = session else { return };
    match state.api.list_explanation_types(session.employee_id).await {
        Ok(resp) => {
            crate::domain::service::explanation_type_service::store_response(&resp);
            tracing::info!(
                version = resp.version,
                count = resp.types.len(),
                "CMS 후 explanation_types 캐시 즉시 갱신"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "CMS 후 explanation_types 재조회 실패 — 다음 user_info_sync 주기 대기");
        }
    }
}

#[cfg(debug_assertions)]
fn cms_begin(form: &ExplanationCmsForm, label: &str) {
    if let Ok(mut p) = form.pending.lock() {
        *p = true;
    }
    if let Ok(mut s) = form.status.lock() {
        *s = Some(format!("{label} 호출 중…"));
    }
}

#[cfg(debug_assertions)]
fn submit_explanation_create(state: &Arc<AppState>, form: &mut ExplanationCmsForm) {
    let Some((requester, cmpsid)) = cms_session_keys(state) else {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("로그인 후 호출 가능합니다.".into());
        }
        return;
    };
    if form.create_label.trim().is_empty() {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("라벨은 비울 수 없습니다.".into());
        }
        return;
    }
    let icon = if form.create_icon.trim().is_empty() {
        None
    } else {
        Some(form.create_icon.trim().to_string())
    };
    let req = crate::data::dto::CreateExplanationTypeRequest {
        cmpsid,
        requester_emp_sid: requester,
        label: form.create_label.trim().to_string(),
        sort_order: form.create_sort,
        icon,
        requires_text: form.create_requires_text,
    };
    cms_begin(form, "추가");
    let state2 = state.clone();
    let status = form.status.clone();
    let pending = form.pending.clone();
    state.runtime.spawn(async move {
        let r = state2.api.create_explanation_type(req).await;
        let msg = match &r {
            Ok(t) => format!(
                "추가 OK — exptype_sid={}, code={}, label={}, sort={}, requires_text={}\n(이 sid 를 비활성 카드에 입력하면 비활성화 가능)",
                t.exptype_sid.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                t.code, t.label, t.sort_order, t.requires_text
            ),
            Err(e) => format!("추가 실패 — {}", humanize_cms_error(e)),
        };
        if r.is_ok() {
            refresh_explanation_cache(&state2).await;
        }
        if let Ok(mut p) = pending.lock() {
            *p = false;
        }
        if let Ok(mut s) = status.lock() {
            *s = Some(msg);
        }
    });
}

#[cfg(debug_assertions)]
fn submit_explanation_patch(state: &Arc<AppState>, form: &mut ExplanationCmsForm) {
    let Some(requester) = cms_requester_emp_sid(state) else {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("로그인 후 호출 가능합니다.".into());
        }
        return;
    };
    let req = crate::data::dto::PatchExplanationTypeRequest {
        requester_emp_sid: requester,
        label: (!form.patch_label.trim().is_empty()).then(|| form.patch_label.trim().to_string()),
        sort_order: (form.patch_sort != 0).then_some(form.patch_sort),
        icon: (!form.patch_icon.trim().is_empty()).then(|| form.patch_icon.trim().to_string()),
        requires_text: Some(form.patch_requires_text),
    };
    let sid = form.patch_sid;
    cms_begin(form, "수정");
    let state2 = state.clone();
    let status = form.status.clone();
    let pending = form.pending.clone();
    state.runtime.spawn(async move {
        let r = state2.api.update_explanation_type(sid, req).await;
        let msg = match &r {
            Ok(t) => format!(
                "수정 OK — sid={sid}, code={}, label={}, sort={}, requires_text={}, is_system={}",
                t.code, t.label, t.sort_order, t.requires_text, t.is_system
            ),
            Err(e) => format!("수정 실패 — {}", humanize_cms_error(e)),
        };
        if r.is_ok() {
            refresh_explanation_cache(&state2).await;
        }
        if let Ok(mut p) = pending.lock() {
            *p = false;
        }
        if let Ok(mut s) = status.lock() {
            *s = Some(msg);
        }
    });
}

#[cfg(debug_assertions)]
fn submit_explanation_deactivate(state: &Arc<AppState>, form: &mut ExplanationCmsForm) {
    let Some((requester, cmpsid)) = cms_session_keys(state) else {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("로그인 후 호출 가능합니다.".into());
        }
        return;
    };
    let req = crate::data::dto::DeactivateExplanationTypeRequest {
        cmpsid,
        requester_emp_sid: requester,
    };
    let sid = form.deactivate_sid;
    cms_begin(form, "비활성");
    let state2 = state.clone();
    let status = form.status.clone();
    let pending = form.pending.clone();
    state.runtime.spawn(async move {
        let r = state2.api.deactivate_explanation_type(sid, req).await;
        let msg = match &r {
            Ok(()) => format!("비활성 OK — sid={sid}"),
            Err(e) => format!("비활성 실패 — {}", humanize_cms_error(e)),
        };
        if r.is_ok() {
            refresh_explanation_cache(&state2).await;
        }
        if let Ok(mut p) = pending.lock() {
            *p = false;
        }
        if let Ok(mut s) = status.lock() {
            *s = Some(msg);
        }
    });
}

#[cfg(debug_assertions)]
fn submit_explanation_usage(state: &Arc<AppState>, form: &mut ExplanationCmsForm) {
    let Some(requester) = cms_requester_emp_sid(state) else {
        if let Ok(mut s) = form.status.lock() {
            *s = Some("로그인 후 호출 가능합니다.".into());
        }
        return;
    };
    let days = form.usage_days;
    cms_begin(form, "통계");
    let state2 = state.clone();
    let status = form.status.clone();
    let pending = form.pending.clone();
    state.runtime.spawn(async move {
        let r = state2.api.get_explanation_usage(requester, days).await;
        if let Ok(mut p) = pending.lock() {
            *p = false;
        }
        let msg = match r {
            Ok(rows) if rows.is_empty() => format!("통계 OK — 지난 {days}일, 사용 0건"),
            Ok(rows) => {
                let mut out = format!("통계 OK — 지난 {days}일:\n");
                for r in rows.iter().take(20) {
                    out.push_str(&format!(
                        "  · {} ({}) — count={}, users={}\n",
                        r.label, r.code, r.count, r.distinct_users
                    ));
                }
                out
            }
            Err(e) => format!("통계 실패 — {}", humanize_cms_error(&e)),
        };
        if let Ok(mut s) = status.lock() {
            *s = Some(msg);
        }
    });
}
