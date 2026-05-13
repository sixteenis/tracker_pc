//! ============================================================================
//! data::repository::auth_repository — 로그인 / 자동로그인 / 로컬 정리.
//! ============================================================================
//!
//! 책임:
//!   1) `data::api::ApiClient::login` 호출 + DTO 검증 → 도메인 모델 변환
//!      (`authenticate` / `try_auto_authenticate` — 부수효과 없는 순수 인증)
//!   2) 인증 성공 후 별도 단계에서 호출되는 영속화 (`persist_session`)
//!      (DB auth row + keyring + company/team 도메인 캐시 + status 초기화)
//!   3) 로그아웃/로컬 정리 (`clear_local`)
//!
//! 인증과 영속화를 분리한 이유: 로그인 후 `check_pay_use` 가 false 면 어떤
//! 로컬 상태도 남겨선 안 됨 → usecase 가 단계 사이에 게이트를 끼울 수 있게
//! API 호출과 저장을 분리.

use anyhow::{anyhow, Context, Result};
use sha1::{Digest, Sha1};
use tracing::warn;

use crate::app::AppState;
use crate::data::dto::login_dto::{LoginRequestDto, LoginResponseDto};
use crate::data::local::auth_repo::{self, AuthRow};
use crate::domain::model::company::Company;
use crate::domain::model::team::Team;
use crate::domain::model::user::User;
use crate::domain::service::{company_service, team_service};
use crate::platform::credential_store::{self, StoredCredentials};

/// 인증 결과 — usecase 가 추가 검증(요금제 등)을 마친 뒤 `persist_session` 으로 보관.
pub struct Authenticated {
    pub user: User,
    pub company: Company,
    pub team: Option<Team>,
    /// 자동로그인 keyring 저장에 그대로 사용. 평문 비밀번호는 호출자 메모리에서 drop 됨.
    pub password_sha1: String,
}

/// 평문 자격증명으로 인증만 수행. 영속화 / 도메인 캐시 갱신 없음.
pub async fn authenticate(
    state: &AppState,
    email: &str,
    password: &str,
) -> Result<Authenticated> {
    let password_sha1 = sha1_hex(password);
    do_authenticate(state, email, &password_sha1).await
}

/// keyring 의 자격증명으로 자동 인증 시도. 자격증명이 없으면 None.
/// 인증 실패 시 자격증명/DB row 폐기 후 None.
pub async fn try_auto_authenticate(state: &AppState) -> Result<Option<Authenticated>> {
    let row = match auth_repo::get(&state.db)? {
        Some(r) if r.auto_login => r,
        _ => return Ok(None),
    };
    let _ = row;

    let creds = match credential_store::load() {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(None),
        Err(e) => {
            warn!(error = %e, "자격증명 로드 실패 — 수동 로그인 필요");
            return Ok(None);
        }
    };

    match do_authenticate(state, &creds.email, &creds.password_sha1).await {
        Ok(authed) => Ok(Some(authed)),
        Err(e) => {
            warn!(error = %e, "자동 인증 실패 — 자격증명 폐기");
            let _ = credential_store::clear();
            let _ = auth_repo::clear(&state.db);
            Ok(None)
        }
    }
}

/// 인증 결과를 영속화 + 도메인 캐시 / status 갱신. usecase 가 모든 검증을
/// 통과한 뒤 호출.
pub fn persist_session(state: &AppState, authed: &Authenticated, auto_login: bool) -> Result<()> {
    auth_repo::upsert(
        &state.db,
        &AuthRow {
            company_id: authed.user.company_id_str.clone(),
            employee_id: authed.user.employee_id_str.clone(),
            employee_name: authed.user.display_name.clone(),
            team_id: authed.user.team_id_str.clone(),
            team_name: authed.team.as_ref().map(|t| t.name.clone()),
            device_id: state.device.device_id.clone(),
            device_name: state.device.device_name.clone(),
            auto_login,
        },
    )?;

    if auto_login {
        if let Err(e) = credential_store::save(&StoredCredentials {
            email: authed.user.email.clone(),
            password_sha1: authed.password_sha1.clone(),
        }) {
            warn!(error = %e, "자격증명 저장 실패 — 자동로그인 다음 실행 시 동작 안 할 수 있음");
        }
    } else {
        let _ = credential_store::clear();
    }

    company_service::set(authed.company.clone());
    if let Some(t) = &authed.team {
        team_service::set(t.clone());
    } else {
        team_service::clear();
    }

    {
        let mut s = state.status.write().unwrap();
        s.can_track_time = authed.user.can_track_time();
        s.attendance = crate::data::dto::AttendanceStatus::Unknown;
    }

    Ok(())
}

/// 로컬 보관물(DB auth row + 회사/팀 캐시) 정리. 도메인 service 가 호출자.
pub fn clear_local(state: &AppState) -> Result<()> {
    auth_repo::clear(&state.db)?;
    company_service::clear();
    team_service::clear();
    Ok(())
}

// ─────────────────────────── 내부 ───────────────────────────

async fn do_authenticate(
    state: &AppState,
    email: &str,
    password_sha1: &str,
) -> Result<Authenticated> {
    let req = LoginRequestDto {
        email: email.to_string(),
        password_sha1: password_sha1.to_string(),
        device_model: state.device.device_name.clone(),
        app_version: state.config.app.app_version.clone(),
    };
    let dto = state.api.login(req).await.context("로그인 요청 실패")?;

    if !dto.is_success() {
        return Err(anyhow!(
            "로그인 실패 (mbrsid={}) — 이메일 또는 비밀번호를 확인해 주세요",
            dto.mbrsid
        ));
    }

    Ok(Authenticated {
        user: to_user(&dto, email),
        company: to_company(&dto),
        team: to_team(&dto),
        password_sha1: password_sha1.to_string(),
    })
}

/// 평문 비밀번호 → SHA-1 hex (40자 소문자).
fn sha1_hex(pw: &str) -> String {
    let mut h = Sha1::new();
    h.update(pw.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn to_user(dto: &LoginResponseDto, login_email: &str) -> User {
    let display_name = (!dto.name.is_empty()).then(|| dto.name.clone());
    let english_name = (!dto.enname.is_empty()).then(|| dto.enname.clone());
    let position = (!dto.spot.is_empty()).then(|| dto.spot.clone());
    let employee_number = (!dto.empnum.is_empty()).then(|| dto.empnum.clone());
    let phone = (!dto.phonenum.is_empty()).then(|| dto.phonenum.clone());
    let backup_email = (!dto.bcemail.is_empty()).then(|| dto.bcemail.clone());
    let profile_image_url = (!dto.profimg.is_empty()).then(|| dto.profimg.clone());
    let join_date = (!dto.joindt.is_empty()).then(|| dto.joindt.clone());
    let registered_date = (!dto.regdt.is_empty()).then(|| dto.regdt.clone());
    let birth_date = (!dto.birth.is_empty()).then(|| dto.birth.clone());
    let update_message = (!dto.updatemsg.is_empty()).then(|| dto.updatemsg.clone());

    // 응답 email 비어있을 수 있으니 로그인 시 입력값을 fallback.
    let email = if dto.email.is_empty() {
        login_email.to_string()
    } else {
        dto.email.clone()
    };

    User {
        member_id: dto.mbrsid,
        employee_id: dto.empsid,
        company_id: dto.cmpsid,
        team_id: (dto.temsid > 0).then_some(dto.temsid),
        team_template_id: dto.ttmsid,
        employee_id_str: dto.empsid.to_string(),
        company_id_str: dto.cmpsid.to_string(),
        team_id_str: (dto.temsid > 0).then(|| dto.temsid.to_string()),
        email,
        display_name: display_name.clone(),
        employee_name: display_name,
        english_name,
        position,
        employee_number,
        phone,
        backup_email,
        profile_image_url,
        authority: dto.author,
        join_date,
        registered_date,
        birth_date,
        update_recommended: dto.update != 0,
        update_message,
    }
}

fn to_company(dto: &LoginResponseDto) -> Company {
    Company {
        id: dto.cmpsid,
        name: if dto.cmpname.is_empty() { "—".to_string() } else { dto.cmpname.clone() },
    }
}

fn to_team(dto: &LoginResponseDto) -> Option<Team> {
    if dto.temsid <= 0 {
        return None;
    }
    Some(Team {
        id: dto.temsid,
        name: if dto.temname.is_empty() { "—".to_string() } else { dto.temname.clone() },
    })
}
