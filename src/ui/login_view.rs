// ============================================================================
// ui::login_view — 로그인 화면 (오렌지 원형 장식 + 흰 카드).
// ============================================================================
//
// 입력: 아이디 / 비밀번호 / 자동로그인 체크박스. 비밀번호는 메모리에서만 사용.
// 로그인 비동기 처리 중에는 버튼이 "로그인 중…" 으로 비활성화.
//
// TODO(2차): "비밀번호 찾기" 링크가 표시만 되고 동작 안 함. 핀플 웹사이트로 외부
// 브라우저 열기 (`webbrowser::open`) 또는 인앱 비밀번호 재설정 화면 추가.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::app::AppState;
use crate::constants;
use crate::ui::{GRAY_TEXT, NAVY, ORANGE, Route};

/// 로그인 입력 상태. `PinpleApp` 에 한 번 보관되며 실패 시 에러 메시지 유지.
#[derive(Default)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub auto_login: bool,
    pub error: Arc<Mutex<Option<String>>>,
    pub busy: Arc<std::sync::atomic::AtomicBool>,
}

/// 로그인 화면 렌더링. 매 프레임 호출됨.
pub fn ui(ui: &mut egui::Ui, state: &Arc<AppState>, form: &mut LoginForm, route: &mut Route) {
    let rect = ui.max_rect();

    // ── 배경 오렌지 원형 장식 ──────────────────────────────────────
    let painter = ui.painter_at(rect);
    painter.circle_filled(
        egui::pos2(rect.left() - 20.0, rect.center().y + 110.0),
        310.0,
        ORANGE,
    );
    painter.circle_filled(
        egui::pos2(rect.left() + 270.0, rect.top() - 30.0),
        185.0,
        ORANGE,
    );

    // ── 중앙 카드 ──────────────────────────────────────────────────
    let card_w = 390.0;
    let avail_h = rect.height();
    let top_pad = ((avail_h - 500.0) / 2.0).max(20.0);

    ui.add_space(top_pad);
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::WHITE)
            .rounding(egui::Rounding::same(16.0))
            .inner_margin(egui::Margin::symmetric(36.0, 32.0))
            .show(ui, |ui| {
                ui.set_width(card_w);

                // 로고
                ui.vertical_centered(|ui| {
                    let (logo_rect, _) = ui.allocate_exact_size(
                        egui::vec2(52.0, 52.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle_filled(logo_rect.center(), 26.0, ORANGE);

                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(constants::APP_FULL_TITLE)
                            .size(26.0)
                            .color(NAVY)
                            .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("핀플 근로자 계정으로 로그인하세요.")
                            .size(13.0)
                            .color(GRAY_TEXT),
                    );
                    ui.label(
                        egui::RichText::new("PC 사용시간이 근무기록 보완 데이터로 기록됩니다.")
                            .size(13.0)
                            .color(GRAY_TEXT),
                    );
                });

                ui.add_space(22.0);

                // 이메일
                ui.label(egui::RichText::new("이메일").size(13.0).color(NAVY).strong());
                ui.add_space(3.0);
                ui.add(
                    egui::TextEdit::singleline(&mut form.email)
                        .hint_text("worker@example.com")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(12.0, 10.0)),
                );

                ui.add_space(10.0);

                // 비밀번호
                ui.label(egui::RichText::new("비밀번호").size(13.0).color(NAVY).strong());
                ui.add_space(3.0);
                ui.add(
                    egui::TextEdit::singleline(&mut form.password)
                        .password(true)
                        .hint_text("••••••••")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(12.0, 10.0)),
                );

                ui.add_space(10.0);

                // 자동로그인
                ui.horizontal(|ui| {
                    let style = ui.style_mut();
                    style.visuals.widgets.inactive.bg_fill =
                        egui::Color32::from_rgb(230, 68, 32);
                    ui.checkbox(&mut form.auto_login, "");
                    ui.label(
                        egui::RichText::new("자동로그인 유지")
                            .size(13.0)
                            .color(NAVY),
                    );
                });

                ui.add_space(14.0);

                // 로그인 버튼
                let busy = form.busy.load(std::sync::atomic::Ordering::Relaxed);
                let btn_text = if busy { "로그인 중…" } else { "로그인" };
                let btn_color = if busy {
                    egui::Color32::from_rgb(180, 100, 70)
                } else {
                    ORANGE
                };
                let btn_w = ui.available_width();
                let enabled = !busy && !form.email.is_empty() && !form.password.is_empty();
                let btn = egui::Button::new(
                    egui::RichText::new(btn_text)
                        .size(15.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(btn_color)
                .rounding(egui::Rounding::same(8.0))
                .min_size(egui::vec2(btn_w, 48.0));

                if ui.add_enabled(enabled, btn).clicked() {
                    start_login(state, form);
                }

                // 에러
                if let Ok(err) = form.error.lock() {
                    if let Some(e) = err.as_ref() {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::from_rgb(210, 50, 50), e);
                    }
                }

                ui.add_space(10.0);

                // 하단 링크
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("비밀번호 찾기")
                            .size(12.0)
                            .color(ORANGE),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(format!("v{}", state.config.app.app_version))
                                    .size(12.0)
                                    .color(GRAY_TEXT),
                            );
                        },
                    );
                });
            });
    });

    // 하단 안내
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("키보드/마우스 입력 내용은 저장되지 않습니다.")
                .size(11.0)
                .color(GRAY_TEXT),
        );
        ui.label(
            egui::RichText::new("입력 발생 여부와 미사용 시간만 기록됩니다.")
                .size(11.0)
                .color(GRAY_TEXT),
        );
    });

    if state.is_logged_in() {
        *route = if state.can_track_time() { Route::Status } else { Route::Disabled };
    }
}

/// 로그인 버튼 클릭 시 호출. 비밀번호를 form 에서 take() 해서 비동기 task 로 전달
/// — 함수 종료와 함께 form.password 는 빈 문자열이 되며, 비동기 task 종료 시
/// 그 안의 pw 도 drop. 평문 비밀번호는 어디에도 영구 저장되지 않는다.
fn start_login(state: &Arc<AppState>, form: &mut LoginForm) {
    let state = state.clone();
    let email = form.email.trim().to_string();
    let pw = std::mem::take(&mut form.password);
    let auto = form.auto_login;
    let err_slot = form.error.clone();
    let busy = form.busy.clone();
    busy.store(true, std::sync::atomic::Ordering::Relaxed);

    let runtime = state.runtime.clone();
    runtime.spawn(async move {
        let result = crate::domain::service::user_service::login(&state, &email, &pw, auto).await;
        if let Ok(mut e) = err_slot.lock() {
            *e = result.as_ref().err().map(|err| err.to_string());
        }
        busy.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}
