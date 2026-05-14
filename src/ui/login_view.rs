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
    /// 비밀번호 평문 표시 토글. true 면 마스킹 해제, false 면 시크릿(점) 표시.
    pub show_password: bool,
    /// 인라인 에러 메시지 (이메일/비밀번호 오류, 네트워크 실패 등).
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

                // 부모 ui 가 `vertical_centered` 상속으로 Align::Center 라서
                // 단순 `ui.label` 은 라벨이 중앙에 떠 버린다. 라벨을 풀폭 영역
                // 안에 좌측 정렬로 배치해 TextEdit 의 왼쪽 끝과 맞춘다.
                let row_width = ui.available_width();
                let left_align_row = egui::Layout::left_to_right(egui::Align::Center);

                // 이메일
                ui.allocate_ui_with_layout(
                    egui::vec2(row_width, 0.0),
                    left_align_row,
                    |ui| {
                        ui.label(
                            egui::RichText::new("이메일")
                                .size(13.0)
                                .color(NAVY)
                                .strong(),
                        );
                    },
                );
                ui.add_space(3.0);
                let email_resp = ui.add(
                    egui::TextEdit::singleline(&mut form.email)
                        .id_source("login_email_input")
                        .hint_text("worker@example.com")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(12.0, 10.0)),
                );

                ui.add_space(10.0);

                // 비밀번호 — id_source 를 명시해 위젯 트리 변동(에러 표시/버튼 라벨 등)
                // 에도 동일한 ID 가 유지되도록 한다. 명시적 ID 가 없으면 매 프레임
                // 자동 생성된 ID 가 흔들려 빠른 타이핑 시 키 이벤트가 누락된다.
                ui.allocate_ui_with_layout(
                    egui::vec2(row_width, 0.0),
                    left_align_row,
                    |ui| {
                        ui.label(
                            egui::RichText::new("비밀번호")
                                .size(13.0)
                                .color(NAVY)
                                .strong(),
                        );
                    },
                );
                ui.add_space(3.0);
                // 비밀번호 입력란 + 오른쪽 눈 토글. horizontal 안에 둘을 나란히
                // 배치하고 응답을 바깥으로 꺼내 onsubmit / 토글 처리에 재사용.
                let eye_size = 28.0;
                let eye_gap = 8.0;
                let pw_field_width = row_width - eye_size - eye_gap;
                let pw_row = ui.allocate_ui_with_layout(
                    egui::vec2(row_width, 0.0),
                    left_align_row,
                    |ui| {
                        let pw_resp = ui.add(
                            egui::TextEdit::singleline(&mut form.password)
                                .id_source("login_password_input")
                                .password(!form.show_password)
                                .hint_text("••••••••")
                                .desired_width(pw_field_width)
                                .margin(egui::vec2(12.0, 10.0)),
                        );
                        ui.add_space(eye_gap);
                        let eye_resp = paint_eye_toggle(ui, form.show_password, eye_size);
                        if eye_resp.clicked() {
                            form.show_password = !form.show_password;
                        }
                        pw_resp
                    },
                );
                let pw_resp = pw_row.inner;

                // Enter 키 제출: 이메일 필드에서 Enter → 비밀번호로 포커스 이동,
                // 비밀번호 필드에서 Enter → 로그인 트리거.
                let enter_pressed =
                    ui.input(|i| i.key_pressed(egui::Key::Enter));
                if email_resp.lost_focus() && enter_pressed {
                    pw_resp.request_focus();
                }
                let submit_via_enter =
                    pw_resp.lost_focus() && enter_pressed && !form.password.is_empty();

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

                if (ui.add_enabled(enabled, btn).clicked() || (submit_via_enter && enabled))
                    && !busy
                {
                    start_login(state, form);
                }

                // 에러
                ui.add_space(10.0);

                // 하단 한 줄에 에러(카드 중앙)와 버전(우측 끝)을 같은 Y 라인에 배치.
                // horizontal 로는 중앙+우측 혼합 정렬이 까다로워서, 한 줄 영역을
                // 통째로 할당해 painter 로 절대 위치에 그린다.
                let bottom_row_h = 18.0;
                let bottom_row_w = ui.available_width();
                let (bottom_row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(bottom_row_w, bottom_row_h),
                    egui::Sense::hover(),
                );
                let bottom_painter = ui.painter_at(bottom_row_rect);

                // 에러 — 있을 때만 카드 중앙에.
                if let Ok(err) = form.error.lock() {
                    if let Some(e) = err.as_ref() {
                        bottom_painter.text(
                            bottom_row_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            e,
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgb(210, 50, 50),
                        );
                    }
                }

                // 버전 — 우측 끝 정렬.
                bottom_painter.text(
                    bottom_row_rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    format!("v{}", state.config.app.app_version),
                    egui::FontId::proportional(12.0),
                    GRAY_TEXT,
                );
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
        // PIN+ 미사용도 메인 화면으로 진입시키고, 메인 화면에서 안내 팝업을 띄운다.
        *route = Route::Status;
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
        // UI 메모리 캐시(소명 내역) 사전 비움 — 이전 사용자 데이터 노출 방지.
        // 도메인 캐시는 user_usecase::login 내부 finalize_session 가 일괄 clear.
        crate::ui::explanation_list_view::clear_cache();
        let result = crate::domain::usecase::user_usecase::login(&state, &email, &pw, auto).await;
        if let Ok(mut e) = err_slot.lock() {
            *e = result.as_ref().err().map(|err| err.to_string());
        }
        busy.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}

/// 눈 모양 토글 아이콘. `shown=true` 면 슬래시가 더해진 "표시 중" 상태(다시 누르면
/// 시크릿 모드로 복귀), `shown=false` 면 일반 눈(누르면 평문 노출). 한글 시스템
/// 폰트에 이모지가 없을 수 있어 painter 로 직접 그린다.
fn paint_eye_toggle(ui: &mut egui::Ui, shown: bool, size: f32) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let color = if resp.hovered() { ORANGE } else { GRAY_TEXT };
    let stroke = egui::Stroke::new(1.6, color);

    // 눈 외곽 — 가로로 긴 둥근 사각형으로 아몬드 모양 흉내.
    let outer = egui::Rect::from_center_size(
        center,
        egui::vec2(size * 0.72, size * 0.46),
    );
    painter.rect_stroke(outer, egui::Rounding::same(size * 0.23), stroke);

    // 동공
    painter.circle_filled(center, size * 0.12, color);

    // 평문 표시 중일 때만 슬래시(/) 를 덧붙여 "다음 클릭 시 숨김" 임을 시사.
    if shown {
        painter.line_segment(
            [
                center + egui::vec2(-size * 0.34, size * 0.30),
                center + egui::vec2(size * 0.34, -size * 0.30),
            ],
            egui::Stroke::new(2.0, color),
        );
    }

    resp
}
