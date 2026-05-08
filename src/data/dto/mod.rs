//! ============================================================================
//! data::dto — 서버 wire 포맷 (raw, 직렬화 전용).
//! ============================================================================
//!
//! 여기 타입들은 약어 필드(`mbrsid`, `cmpsid`, `temsid` 등) 를 그대로 들고 있다.
//! `data` 레이어 밖으로 노출하지 않는다 — `repository` 가 도메인 모델로
//! 변환한 뒤에만 외부로 흘러간다.
//!
//! TODO(2차): 현재는 `_legacy_types.rs` 에 기존 api/types.rs 를 통째로 옮겨두었음.
//! 신규 서버 명세가 들어오는 엔드포인트부터 순차적으로 별도 파일(`login_dto.rs`,
//! `policy_dto.rs` 등) 로 분리.

pub mod login_dto;

// 임시: 정책/heartbeat/이벤트/소명/출근/업데이트 DTO 는 아직 신규 서버 명세 미정.
// 기존 타입들을 그대로 재export 해서 `data::api` 와 `repository` 가 사용한다.
mod _legacy_types;
pub use _legacy_types::{
    AttendanceSnapshot, AttendanceStatus, EventEntry, EventsBatch, EventsBatchResponse,
    ExplanationSubmit, ExplanationType, HeartbeatRequest, HeartbeatResponse, PolicySnapshot,
    RemoteExplanation, UpdateCheckRequest, UpdateInfo,
};
