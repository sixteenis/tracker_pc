//! ============================================================================
//! platform — OS 의존성 / 백그라운드 작업 / 런타임 인프라.
//! ============================================================================
//!
//! - `credential_store` : OS Credential Store (keyring) 자격증명 보관
//! - `device`           : 디바이스 식별자 / 모델명 영속화
//! - `monitor`          : 입력 / 잠금 / 자리비움 감지 백그라운드 task
//! - `notify`           : 토스트 알림
//! - `sync`             : 서버 동기화 백그라운드 task (heartbeat / events / policy / attendance / update)

pub mod credential_store;
pub mod device;
pub mod monitor;
pub mod notify;
pub mod sync;
