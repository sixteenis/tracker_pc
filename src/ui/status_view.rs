// ============================================================================
// ui::status_view — 메인 상태 화면 (로그인 후 기본 화면).
// ============================================================================
//
// 구성:
//   1) 오렌지 헤더 — 사번/팀명 + PC 상태 배지
//   2) 출근 카드(흰) + 출퇴근 안내 카드(네이비, 소명 N건 진입 버튼)
//   3) 통계 카드 2개 — 오늘 PC 사용 / 오늘 미사용 누적
//   4) 활동 감지 타임라인 — 09:00 ~ 현재까지 시각화
//   5) 하단 버튼 3개 — 적용된 정책 / 지금 동기화 / 오늘 기록 보기
//   6) 개인정보 안내 한 줄
//
// TODO(미구현): "지금 동기화" 버튼이 로그만 찍고 실제 동기화 트리거 안 함.
// `sync` 레이어에 mpsc 채널 추가해서 즉시 트리거되게 해야 함.
// TODO(미구현): "적용된 정책 (관리자 설정)" 토글 펼치기 — 펼침 영역에 회사/팀/개인
// 기준 시간 표 + 우선순위 표시 (현재는 헤더만 있고 펼쳐도 비어있음 가능).
// TODO(2차): 타임라인 시작 시각이 09:00 으로 하드코딩 — 출근 시각 기준으로 동적 변경.

use std::sync::Arc;

use chrono::{Local, Timelike, Utc};
use eframe::egui;

use crate::app::{AppState, PcStatus};
use crate::data::local::idle_segments_repo::{self, IdleSegment, SegmentType};
use crate::ui::{BG, GRAY_TEXT, GREEN_STATUS, NAVY, ORANGE, TIMELINE_ACTIVE, TIMELINE_IDLE, TIMELINE_LOCKED, Route};
use crate::util;

/// 메인 상태 화면 렌더링. 매 프레임 호출.
pub fn show(ctx: &egui::Context, state: &Arc<AppState>, route: &mut Route) {
    let snapshot = state.snapshot_status();
    let session = state.session.read().unwrap().clone();
    let today = Utc::now().date_naive();

    let today_segments = session
        .as_ref()
        .and_then(|s| idle_segments_repo::list_for_date(&state.db, &s.employee_id_str, today).ok())
        .unwrap_or_default();

    let pending_count = today_segments
        .iter()
        .filter(|s| {
            matches!(
                s.explanation_status,
                idle_segments_repo::ExplanationStatus::Pending
                    | idle_segments_repo::ExplanationStatus::Expired
            )
        })
        .count();

    // ── 오렌지 헤더 ───────────────────────────────────────────────
    egui::TopBottomPanel::top("status_header")
        .frame(
            egui::Frame::none()
                .fill(ORANGE)
                .inner_margin(egui::Margin::symmetric(24.0, 14.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("FINNQ · PC AGENT")
                            .size(11.0)
                            .color(egui::Color32::from_rgba_premultiplied(255, 255, 255, 180)),
                    );
                    ui.add_space(2.0);
                    let team_name = crate::domain::service::team_service::current()
                        .map(|t| t.name)
                        .unwrap_or_default();
                    let name = session
                        .as_ref()
                        .map(|s| {
                            let n = s.name_for_display();
                            format!("{n} {team_name}").trim().to_string()
                        })
                        .unwrap_or_else(|| "—".to_string());
                    ui.label(
                        egui::RichText::new(name).size(22.0).color(egui::Color32::WHITE).strong(),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, dot_color) = match snapshot.pc_status {
                        PcStatus::Active => ("PC 사용 중", GREEN_STATUS),
                        PcStatus::Idle => ("자리비움", TIMELINE_IDLE),
                        PcStatus::Locked => ("잠금 중", GRAY_TEXT),
                        PcStatus::Offline => ("오프라인", egui::Color32::from_rgb(200, 80, 80)),
                        PcStatus::AppClosing => ("종료 중", egui::Color32::from_rgb(200, 80, 80)),
                    };
                    egui::Frame::none()
                        .fill(egui::Color32::WHITE)
                        .rounding(egui::Rounding::same(20.0))
                        .inner_margin(egui::Margin::symmetric(14.0, 7.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.painter().circle_filled(
                                    ui.cursor().min + egui::vec2(5.0, 7.0),
                                    5.0,
                                    dot_color,
                                );
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new(label).size(13.0).color(NAVY).strong(),
                                );
                            });
                        });
                });
            });
        });

    // ── 메인 콘텐츠 ───────────────────────────────────────────────
    // Frame::inner_margin 으로 감싸서 모든 자식이 동일한 available_width = W-40 을 받음.
    // ui.horizontal+add_space 방식은 vertical 내부에서 available_width = W-20 을 리턴하므로
    // 타임라인 같은 full-width 요소가 right padding 을 무시하게 됨.
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(BG))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let gap = 8.0;

                    egui::Frame::none()
                        .inner_margin(egui::Margin::same(20.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            // 이제 ui.available_width() = W - 40 (좌우 20px 패딩 적용)
                            let content_w = ui.available_width();
                            {

                    // ── 출근/퇴근 카드 ────────────────────────────
                    let card1_w = 200.0_f32.min(content_w * 0.24);
                    let card2_w = content_w - card1_w - gap;
                    let attendance_label = snapshot.attendance.label();
                    let card_row_h = 68.0; // 두 카드 동일 높이

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        // 출근 카드
                        ui.allocate_ui(egui::vec2(card1_w, card_row_h), |ui| {
                            egui::Frame::none()
                                .fill(egui::Color32::WHITE)
                                .rounding(egui::Rounding::same(10.0))
                                .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("출근").size(12.0).color(GRAY_TEXT));
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(attendance_label)
                                                .size(17.0)
                                                .color(NAVY)
                                                .strong(),
                                        );
                                    });
                                });
                        });

                        ui.add_space(gap);

                        // 퇴근 / 소명 카드
                        ui.allocate_ui(egui::vec2(card2_w, card_row_h), |ui| {
                            egui::Frame::none()
                                .fill(NAVY)
                                .rounding(egui::Rounding::same(10.0))
                                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new("출퇴근 기록")
                                                        .size(12.0)
                                                        .color(egui::Color32::from_rgba_premultiplied(255, 255, 255, 160)),
                                                );
                                                ui.label(
                                                    egui::RichText::new("스마트폰 앱에서 처리")
                                                        .size(14.0)
                                                        .color(egui::Color32::WHITE),
                                                );
                                            });
                                            if pending_count > 0 {
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    egui::RichText::new(format!(
                                                                        "소명 {pending_count}건 ▶"
                                                                    ))
                                                                    .size(13.0)
                                                                    .color(NAVY)
                                                                    .strong(),
                                                                )
                                                                .fill(egui::Color32::WHITE)
                                                                .rounding(egui::Rounding::same(6.0)),
                                                            )
                                                            .clicked()
                                                        {
                                                            *route = Route::ExplanationList;
                                                        }
                                                    },
                                                );
                                            }
                                        },
                                    );
                                });
                        });
                    });

                    ui.add_space(10.0);

                    // ── 통계 카드 2개 ──────────────────────────────
                    let total_active = {
                        let now = Utc::now();
                        let local_now = now.with_timezone(&Local);
                        let midnight = local_now.date_naive().and_hms_opt(0, 0, 0).unwrap();
                        let midnight_utc = chrono::TimeZone::from_local_datetime(
                            &chrono::FixedOffset::east_opt(
                                Local::now().offset().local_minus_utc(),
                            )
                            .unwrap(),
                            &midnight,
                        )
                        .single()
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(now - chrono::Duration::hours(8));
                        let elapsed = (now - midnight_utc).num_seconds().max(0);
                        let idle_total: i64 =
                            today_segments.iter().filter_map(|s| s.duration_seconds).sum();
                        (elapsed - idle_total).max(0)
                    };
                    let idle_count = today_segments.len();
                    let max_idle = today_segments
                        .iter()
                        .filter_map(|s| s.duration_seconds)
                        .max()
                        .unwrap_or(0);

                    let stat_row_h = 82.0; // 두 카드 동일 높이
                    let half_w = (content_w - gap) / 2.0;

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.allocate_ui(egui::vec2(half_w, stat_row_h), |ui| {
                            stat_card(
                                ui,
                                "오늘 PC 사용",
                                &format_active(total_active),
                                "오늘 감지 시작",
                            );
                        });
                        ui.add_space(gap);
                        ui.allocate_ui(egui::vec2(half_w, stat_row_h), |ui| {
                            let idle_label = if idle_count > 0 {
                                format!(
                                    "{}  ·{}회",
                                    util::format_duration_human(snapshot.idle_seconds as i64),
                                    idle_count
                                )
                            } else {
                                "—".to_string()
                            };
                            let idle_sub = if max_idle > 0 {
                                format!("최장 이탈 {}", util::format_duration_human(max_idle))
                            } else {
                                "이탈 없음".to_string()
                            };
                            stat_card(ui, "오늘 미사용 누적", &idle_label, &idle_sub);
                        });
                    });

                    ui.add_space(10.0);

                    // ── 활동 감지 타임라인 ─────────────────────────
                    egui::Frame::none()
                        .fill(egui::Color32::WHITE)
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());

                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("활동 감지 상태")
                                        .size(14.0)
                                        .color(NAVY)
                                        .strong(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let sync_text = snapshot
                                            .last_event_sync_at
                                            .as_ref()
                                            .map(|t| {
                                                let l = t.with_timezone(&Local);
                                                format!(
                                                    "서버 연결됨 · 동기화 {:02}:{:02}:{:02}",
                                                    l.hour(),
                                                    l.minute(),
                                                    l.second()
                                                )
                                            })
                                            .unwrap_or_else(|| "동기화 대기 중".to_string());
                                        ui.horizontal(|ui| {
                                            ui.painter().circle_filled(
                                                ui.cursor().min + egui::vec2(5.0, 7.0),
                                                4.0,
                                                GREEN_STATUS,
                                            );
                                            ui.add_space(10.0);
                                            ui.label(
                                                egui::RichText::new(sync_text)
                                                    .size(11.0)
                                                    .color(GRAY_TEXT),
                                            );
                                        });
                                    },
                                );
                            });

                            ui.add_space(10.0);
                            draw_timeline(ui, &today_segments);
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                legend_dot(ui, TIMELINE_ACTIVE, "사용");
                                ui.add_space(8.0);
                                legend_dot(ui, TIMELINE_IDLE, "미사용");
                                ui.add_space(8.0);
                                legend_dot(ui, TIMELINE_LOCKED, "잠금");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "이탈 기준 {}분",
                                                snapshot.effective_idle_threshold_seconds / 60
                                            ))
                                            .size(11.0)
                                            .color(GRAY_TEXT),
                                        );
                                    },
                                );
                            });
                        });

                    ui.add_space(12.0);

                    // ── 하단 버튼 3개 ─────────────────────────────
                    let third = (content_w - gap * 2.0) / 3.0;
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        for (label, is_filled, action) in [
                            ("⚙  적용된 정책", false, 0u8),
                            ("↻  지금 동기화", false, 1),
                            ("오늘 기록 보기", true, 2),
                        ] {
                            let btn = if is_filled {
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .size(13.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(NAVY)
                                .rounding(egui::Rounding::same(8.0))
                                .min_size(egui::vec2(third, 44.0))
                            } else {
                                egui::Button::new(
                                    egui::RichText::new(label).size(13.0).color(NAVY),
                                )
                                .fill(egui::Color32::WHITE)
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    egui::Color32::from_rgb(220, 220, 220),
                                ))
                                .rounding(egui::Rounding::same(8.0))
                                .min_size(egui::vec2(third, 44.0))
                            };
                            if ui.add(btn).clicked() {
                                match action {
                                    0 => *route = Route::Settings,
                                    1 => tracing::info!("수동 동기화 요청"),
                                    _ => *route = Route::ExplanationList,
                                }
                            }
                            if action < 2 {
                                ui.add_space(gap);
                            }
                        }
                    });

                    ui.add_space(8.0);

                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "ⓘ  키보드/마우스 입력 내용은 저장되지 않습니다. 입력 발생 여부와 미사용 시간만 기록됩니다.",
                            )
                            .size(11.0)
                            .color(GRAY_TEXT),
                        );
                    });

                            } // content block
                        }); // Frame::inner_margin
                }); // ScrollArea
        }); // CentralPanel
}

// ── 헬퍼 위젯 ─────────────────────────────────────────────────

/// 통계 카드 한 개 (제목/값/서브). 텍스트 모두 가로 중앙 정렬.
fn stat_card(ui: &mut egui::Ui, title: &str, value: &str, subtitle: &str) {
    egui::Frame::none()
        .fill(egui::Color32::WHITE)
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(title).size(12.0).color(GRAY_TEXT));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(value).size(22.0).color(NAVY).strong());
                ui.add_space(2.0);
                ui.label(egui::RichText::new(subtitle).size(11.0).color(GRAY_TEXT));
            });
        });
}

/// 타임라인 범례용 작은 색상 블럭 + 텍스트.
fn legend_dot(ui: &mut egui::Ui, color: egui::Color32, label: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(r, egui::Rounding::same(2.0), color);
        ui.label(egui::RichText::new(label).size(11.0).color(GRAY_TEXT));
    });
}

/// 09:00 ~ 현재까지 타임라인 막대 (초록=사용 / 주황=미사용 / 회색=잠금).
/// 세그먼트가 양 끝에 닿으면 해당 모서리만 둥글게 마무리.
/// 하단에 2시간 단위 hour 레이블 + 우측 끝 "현재 HH:MM".
fn draw_timeline(ui: &mut egui::Ui, segments: &[IdleSegment]) {
    let bar_h = 26.0;
    let label_pad = 8.0;
    let label_h = 14.0;
    let total_h = bar_h + label_pad + label_h;
    let bar_radius = 6.0;

    // 바 + 레이블 영역을 한꺼번에 할당해서 카드(흰 박스) 내부에서 항상 잘리지 않게.
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), total_h),
        egui::Sense::hover(),
    );
    let bar_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), bar_h));

    let now_local = chrono::Local::now();
    let today_start_h = 9u32;
    let start_local = now_local.date_naive().and_hms_opt(today_start_h, 0, 0).unwrap();
    let start_utc = chrono::TimeZone::from_local_datetime(
        &chrono::FixedOffset::east_opt(now_local.offset().local_minus_utc()).unwrap(),
        &start_local,
    )
    .single()
    .map(|dt| dt.with_timezone(&Utc))
    .unwrap_or_else(Utc::now);

    let now_utc = Utc::now();
    let total_secs = (now_utc - start_utc).num_seconds().max(1) as f64;
    let bar_rounding = egui::Rounding::same(bar_radius);

    let painter = ui.painter();
    // 바 배경 (활성 = 초록, 양쪽 모두 둥글게).
    painter.rect_filled(bar_rect, bar_rounding, TIMELINE_ACTIVE);

    // 자리비움/잠금 구간 오버레이 — 바 끝에 닿으면 그쪽 모서리만 맞춰 둥글게.
    for seg in segments {
        if seg.start_time > now_utc {
            continue;
        }
        let seg_start = (seg.start_time - start_utc).num_seconds().max(0) as f64;
        let seg_end = seg
            .end_time
            .map(|e| (e - start_utc).num_seconds().max(0) as f64)
            .unwrap_or(total_secs);
        let x0 = (bar_rect.left() + (seg_start / total_secs) as f32 * bar_rect.width())
            .max(bar_rect.left());
        let x1 = (bar_rect.left() + (seg_end / total_secs) as f32 * bar_rect.width())
            .min(bar_rect.right());
        if x1 <= x0 {
            continue;
        }
        let sr = egui::Rect::from_min_max(
            egui::pos2(x0, bar_rect.top()),
            egui::pos2(x1, bar_rect.bottom()),
        );
        let touches_left = (x0 - bar_rect.left()).abs() < 0.5;
        let touches_right = (x1 - bar_rect.right()).abs() < 0.5;
        let mut r = egui::Rounding::ZERO;
        if touches_left {
            r.nw = bar_radius;
            r.sw = bar_radius;
        }
        if touches_right {
            r.ne = bar_radius;
            r.se = bar_radius;
        }
        let color = match seg.segment_type {
            SegmentType::PcLocked => TIMELINE_LOCKED,
            _ => TIMELINE_IDLE,
        };
        painter.rect_filled(sr, r, color);
    }

    // 시간 레이블 — 박스 안에 위치, 우측에는 "현재 HH:MM".
    let label_y = bar_rect.bottom() + label_pad;
    let now_h = now_local.hour();
    let now_m = now_local.minute();
    let now_decimal = now_h as f32 + now_m as f32 / 60.0;
    let span = (now_decimal - today_start_h as f32).max(0.5);
    let label_font = egui::FontId::proportional(10.0);

    // 09:00 부터 2시간 단위로 — 단, "현재" 라벨과 겹치지 않게 1시간 이상 차이날 때만.
    let mut h = today_start_h;
    while h < now_h {
        if (now_decimal - h as f32) >= 1.0 {
            let t = ((h - today_start_h) as f32 / span).clamp(0.0, 1.0);
            let x = bar_rect.left() + t * bar_rect.width();
            let align = if h == today_start_h {
                egui::Align2::LEFT_TOP
            } else {
                egui::Align2::CENTER_TOP
            };
            painter.text(
                egui::pos2(x, label_y),
                align,
                format!("{h:02}:00"),
                label_font.clone(),
                GRAY_TEXT,
            );
        }
        h += 2;
    }

    // 우측 끝 — "현재 HH:MM"
    painter.text(
        egui::pos2(bar_rect.right(), label_y),
        egui::Align2::RIGHT_TOP,
        format!("현재 {:02}:{:02}", now_h, now_m),
        label_font,
        GRAY_TEXT,
    );
}

/// 통계 카드의 누적 시간 표기 (예: 4h 12m / 25m).
fn format_active(seconds: i64) -> String {
    let s = seconds.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    if h > 0 { format!("{h}h {m}m") } else { format!("{m}m") }
}
