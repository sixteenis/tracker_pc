//! ============================================================================
//! data::repository::auth_repository — 로그인 / 자동로그인 / 로컬 정리.
//! ============================================================================
//!
//! 책임:
//!   1) `data::api` 의 `ApiClient::login` 호출
//!   2) 응답 DTO (`LoginResponseDto`) 의 `mbrsid` 검증 → 실패 시 에러
//!   3) DTO → 도메인 모델 (`User`, `Company`, `Team`) 변환
//!   4) DB(`data::local::auth_repo`) + keyring(`platform::credential_store`) 영속화
//!   5) 부수 효과로 `domain::service::company_service` / `team_service` 캐시 갱신
//!      (User 응답에 회사/팀 이름이 같이 오므로 한 번에 채워주는 게 자연스러움)

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

/// 평문 자격증명으로 로그인 후 도메인 `User` 반환. 도메인 service 가 호출자.
pub async fn login(
    state: &AppState,
    email: &str,
    password: &str,
    auto_login: bool,
) -> Result<User> {
    let password_sha1 = sha1_hex(password);
    apply_login(state, email, &password_sha1, auto_login).await
}

/// keyring 의 자격증명으로 자동 로그인 시도.
/// 자격증명이 없거나 인증 실패하면 자격증명/DB row 폐기 후 None.
pub async fn try_auto_login(state: &AppState) -> Result<Option<User>> {
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

    match apply_login(state, &creds.email, &creds.password_sha1, true).await {
        Ok(user) => Ok(Some(user)),
        Err(e) => {
            warn!(error = %e, "자동로그인 실패 — 자격증명을 폐기");
            let _ = credential_store::clear();
            let _ = auth_repo::clear(&state.db);
            Ok(None)
        }
    }
}

/// 로컬 보관물(DB auth row + 회사/팀 캐시) 정리. 도메인 service 가 호출자.
pub fn clear_local(state: &AppState) -> Result<()> {
    auth_repo::clear(&state.db)?;
    company_service::clear();
    team_service::clear();
    Ok(())
}

// ─────────────────────────── 내부 ───────────────────────────

async fn apply_login(
    state: &AppState,
    email: &str,
    password_sha1: &str,
    auto_login: bool,
) -> Result<User> {
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

    let user = to_user(&dto, email);
    let company = to_company(&dto);
    let team = to_team(&dto);

    // 로컬 영속화
    auth_repo::upsert(
        &state.db,
        &AuthRow {
            company_id: user.company_id_str.clone(),
            employee_id: user.employee_id_str.clone(),
            employee_name: user.display_name.clone(),
            team_id: user.team_id_str.clone(),
            team_name: team.as_ref().map(|t| t.name.clone()),
            device_id: state.device.device_id.clone(),
            device_name: state.device.device_name.clone(),
            auto_login,
        },
    )?;

    if auto_login {
        if let Err(e) = credential_store::save(&StoredCredentials {
            email: email.to_string(),
            password_sha1: password_sha1.to_string(),
        }) {
            warn!(error = %e, "자격증명 저장 실패 — 자동로그인 다음 실행 시 동작 안 할 수 있음");
        }
    } else {
        let _ = credential_store::clear();
    }

    // 동반 도메인 갱신 — 응답에 같이 들어 있는 회사/팀 정보를 즉시 반영.
    company_service::set(company);
    if let Some(t) = team {
        team_service::set(t);
    } else {
        team_service::clear();
    }

    // can_track_time / attendance 초기 상태 반영
    {
        let mut s = state.status.write().unwrap();
        s.can_track_time = user.can_track_time();
        s.attendance = crate::data::dto::AttendanceStatus::Unknown;
    }

    Ok(user)
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
