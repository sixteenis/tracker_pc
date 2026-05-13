//! ============================================================================
//! data::dto::main_info_dto — `/android/u/get_main2.jsp` 응답 DTO.
//! ============================================================================
//!
//! 서버는 모바일 클라이언트 기준으로 다양한 필드(비콘 / 위치 / 광고 등) 를 함께
//! 보내주지만, 본 PC Agent 가 실제 사용할 만한 항목만 추려 매핑한다.
//! 서버 wire 약어를 그대로 보존하며, 도메인 모델 변환은 repository 가 수행.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MainInfoResponseDto {
    /// 출근시간 ("HH:MM").
    #[serde(default)]
    pub starttm: String,
    /// 퇴근시간 ("HH:MM").
    #[serde(default)]
    pub endtm: String,
    /// 입사일 ("YYYY-MM-DD"). 1900-01-01 이면 입사일 미설정.
    #[serde(default)]
    pub joindt: String,

    /// 남은 연차 (분 단위).
    #[serde(default)]
    pub anual: i64,
    /// 오늘 근로 합계 (분 단위).
    #[serde(default)]
    pub workmin: i64,
    /// 추가 근로 (분 단위).
    #[serde(default)]
    pub addmin: i64,
    /// 사용 근로 (분 단위).
    #[serde(default)]
    pub usemin: i64,

    /// 미확인 메시지 카운트.
    #[serde(default)]
    pub msgcnt: i32,

    /// 지각 연차 차감 (0/1).
    #[serde(default)]
    pub anualddctn1: i32,
    /// 조퇴 연차 차감 (0/1).
    #[serde(default)]
    pub anualddctn2: i32,
    /// 외출 연차 차감 (0/1).
    #[serde(default)]
    pub anualddctn3: i32,

    /// 입사일 기준(1) / 회계년도(0).
    #[serde(default, rename = "stAnual")]
    pub st_anual: i32,
    #[serde(default, rename = "stDAnual")]
    pub st_d_anual: i32,

    /// 휴게시간 사용 (0/1).
    #[serde(default, rename = "brkTime")]
    pub brk_time: i32,
    /// 근무일정 사용 (0/1).
    #[serde(default)]
    pub schdl: i32,
    /// 자동 퇴근기록 설정 (1/2/3).
    #[serde(default, rename = "cmtLt")]
    pub cmt_lt: i32,
    /// 출퇴근 전 알림 (0/1).
    #[serde(default)]
    pub cmtnoti: i32,
    /// 주52시간 단위 (0=주, 1=월).
    #[serde(default)]
    pub wk52h: i32,
}
