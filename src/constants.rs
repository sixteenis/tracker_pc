//! ============================================================================
//! constants.rs — 빌드 시점에 고정되는 전역 상수.
//! ============================================================================
//!
//! 런타임 변경이 일어나지 않는 식별자/브랜드/파일명을 한 곳에서 관리.
//! 런타임 설정값(서버 host, 인터벌, 타임아웃 등)은 `config::AppConfig` /
//! `config/default.toml` 에서 관리한다.
//!
//! 외부에서 사용:
//!   - `crate::constants::APP_ID`
//!   - `crate::constants::KEYRING_SERVICE`
//!   - 등
//!
//! ── 추가/수정 시 주의 ──────────────────────────────────────────────────
//! - APP_QUALIFIER/ORG/NAME 은 ProjectDirs(설정/데이터 디렉토리 경로)와 직결됨.
//!   변경 시 사용자 PC 의 기존 설정/DB 가 새 경로로 마이그레이션되지 않으니 주의.
//! - KEYRING_SERVICE 변경 시 기존 사용자 자동로그인 토큰이 새 키와 매칭되지 않아
//!   다음 실행에서 모두 수동 로그인을 다시 해야 함.

// ─────────────────────────── 1. 앱 식별자 ───────────────────────────

/// ProjectDirs 의 qualifier (역방향 도메인 최상위).
pub const APP_QUALIFIER: &str = "io";

/// ProjectDirs 의 organization.
pub const APP_ORG: &str = "Pinple";

/// ProjectDirs 의 application.
pub const APP_NAME: &str = "PCAgent";

/// 역방향 도메인 형태 앱 ID. keyring service / 단일 인스턴스 락 prefix 로 사용.
pub const APP_ID: &str = "io.pinple.pcagent";

/// 단일 인스턴스 락 ID — 버전 suffix 포함.
/// 메이저 버전 호환성 깨질 때만 v2 등으로 변경.
pub const INSTANCE_LOCK_ID: &str = "io.pinple.pcagent.v1";

/// OS Credential Store(`keyring`) 의 service 이름.
pub const KEYRING_SERVICE: &str = APP_ID;

// ─────────────────────────── 2. 표시 / UX ───────────────────────────

/// 시스템 트레이 / 알림 / 윈도우 타이틀에 노출되는 짧은 제품명.
pub const APP_DISPLAY_NAME: &str = "핀플 PC";

/// 메인 윈도우 타이틀 / 로그인 화면 헤더.
pub const APP_FULL_TITLE: &str = "핀플 PC 에이전트";

// ─────────────────────────── 3. 네트워크 ───────────────────────────

/// HTTP User-Agent prefix. 실제 헤더는 `"{USER_AGENT_PREFIX}/{버전}"`.
pub const USER_AGENT_PREFIX: &str = "PinplePCAgent";

// ─────────────────────────── 4. 파일 / 로컬 저장소 ───────────────────────────

/// `data_dir()` 아래에 생성되는 SQLite 파일명.
pub const DB_FILE_NAME: &str = "pinple.db";

// ─────────────────────────── 5. 로깅 ───────────────────────────

/// `RUST_LOG` 미설정 시 사용되는 자기 크레이트 필터 prefix.
/// `Cargo.toml` `[package].name` 과 일치해야 함.
pub const LOG_CRATE_FILTER: &str = "pinple_pc_agent";

// ─────────────────────────── 6. 도메인 코드 ───────────────────────────

/// 소명 제출 시 `submitted_from` 필드에 항상 들어가는 값.
pub const SUBMITTED_FROM_PC_APP: &str = "PC_APP";

// ─────────────────────────── 7. 로그인 API 파라미터 ───────────────────────────

/// `check_mbr.jsp` 의 OSVS 파라미터. 안드로이드 SDK 코드를 그대로 재사용.
/// PC 측 별도 코드가 정해지면 그 값으로 교체.
pub const LOGIN_OSVS: &str = "33";

/// `check_mbr.jsp` 의 MODE 파라미터. 0 = 일반 로그인.
pub const LOGIN_MODE: &str = "0";
