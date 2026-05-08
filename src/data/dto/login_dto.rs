//! ============================================================================
//! data::dto::login_dto — `/android/check_mbr.jsp` 요청/응답 DTO.
//! ============================================================================
//!
//! 서버 wire 포맷 그대로 — `mbrsid`, `cmpsid` 같은 약어 필드를 들고 있다.
//! 도메인 모델(`domain::model::user::User`) 변환은 `data::repository::auth_repository`
//! 가 수행한다. 이 파일의 타입은 `data` 레이어 밖으로 노출하지 않는다.

use serde::{Deserialize, Serialize};

/// `check_mbr.jsp` 호출용 평문 자격 증명. HTTP 클라이언트가 BASE64/SHA1 변환 후
/// query string 으로 전송한다. 직렬화 대상이 아니므로 `Serialize` 미구현.
#[derive(Debug, Clone)]
pub struct LoginRequestDto {
    /// 사용자 이메일 (평문). 클라이언트에서 BASE64 인코딩 후 EMAIL 파라미터로 전송.
    pub email: String,
    /// 비밀번호 SHA-1 해시 (40자 hex). PASS 파라미터에 그대로 전송.
    /// 자동로그인 시에도 keyring 의 해시를 그대로 재사용한다.
    pub password_sha1: String,
    /// 단말기 모델명. 클라이언트에서 BASE64 인코딩 후 MD 파라미터로 전송.
    pub device_model: String,
    /// 앱 버전 (`config.app.app_version`). APPVS 파라미터.
    pub app_version: String,
}

/// `check_mbr.jsp` 응답을 그대로 매핑한 raw DTO.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginResponseDto {
    #[serde(default)] pub mbrsid: i64,
    #[serde(default)] pub empsid: i64,
    #[serde(default)] pub cmpsid: i64,
    #[serde(default)] pub temsid: i64,
    #[serde(default)] pub ttmsid: i64,

    #[serde(default)] pub email: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub enname: String,
    #[serde(default)] pub ttmname: String,
    #[serde(default)] pub cmpname: String,
    #[serde(default)] pub temname: String,

    #[serde(default)] pub gender: i32,
    #[serde(default)] pub birth: String,
    #[serde(default)] pub phonenum: String,
    #[serde(default)] pub bcemail: String,
    #[serde(default)] pub empnum: String,
    #[serde(default)] pub spot: String,

    #[serde(default)] pub author: i32,
    #[serde(default)] pub lunar: i32,
    #[serde(default)] pub notrc: i32,

    #[serde(default)] pub regdt: String,
    #[serde(default)] pub joindt: String,

    #[serde(default)] pub profimg: String,
    #[serde(default)] pub pushid: String,
    #[serde(default)] pub curver: String,

    #[serde(default)] pub update: i32,
    #[serde(default)] pub updatemsg: String,
}

impl LoginResponseDto {
    /// `mbrsid` 가 양수일 때만 로그인 성공 (0 / -1 등은 인증 실패).
    pub fn is_success(&self) -> bool {
        self.mbrsid > 0
    }
}
