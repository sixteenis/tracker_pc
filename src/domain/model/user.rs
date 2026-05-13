//! ============================================================================
//! domain::model::user — 로그인된 회원의 도메인 모델.
//! ============================================================================
//!
//! 서버 wire 포맷(`mbrsid`, `cmpsid` 등 약어) 은 `data::dto::login_dto::LoginResponseDto`
//! 가 전담하고, UI / 도메인 서비스는 본 `User` 만 본다.

#[derive(Debug, Clone)]
pub struct User {
    /// 회원 고유 ID — 서버 `mbrsid`.
    pub member_id: i64,
    /// 사원 고유 ID — 서버 `empsid`. 출근/이벤트 식별자로 사용.
    pub employee_id: i64,
    /// 회사 고유 ID — 서버 `cmpsid`.
    pub company_id: i64,
    /// 팀 고유 ID — 서버 `temsid`. 미배정이면 None.
    pub team_id: Option<i64>,
    /// 팀 템플릿 ID — 서버 `ttmsid`. get_main2 호출에 필요. 0 이면 미배정.
    pub team_template_id: i64,

    // ── DB / SQL 호환을 위한 String ID (`employee_id.to_string()` 캐시) ──
    /// 로컬 DB 컬럼이 TEXT 라 그대로 비교가 가능하도록 보관. SQL 외 사용 비추천.
    pub employee_id_str: String,
    pub company_id_str: String,
    pub team_id_str: Option<String>,

    /// 이메일 (로그인 ID).
    pub email: String,
    /// 표시 이름 (서버 `name`). 빈 문자열은 None 으로 변환.
    pub display_name: Option<String>,
    /// `display_name` 의 별칭 — 기존 UI 코드 호환용.
    pub employee_name: Option<String>,
    /// 영문 이름 (서버 `enname`). 옵셔널.
    pub english_name: Option<String>,
    /// 직책/직위 (서버 `spot`). 옵셔널.
    pub position: Option<String>,
    /// 사번 (서버 `empnum`). 옵셔널.
    pub employee_number: Option<String>,
    /// 휴대전화 (서버 `phonenum`). 옵셔널.
    pub phone: Option<String>,
    /// 비공식 이메일 (서버 `bcemail`). 옵셔널.
    pub backup_email: Option<String>,
    /// 프로필 이미지 URL (서버 `profimg`). 옵셔널.
    pub profile_image_url: Option<String>,

    /// 권한 코드 (서버 `author`). 의미는 백엔드 합의 후 도메인 enum 으로 분리 예정.
    pub authority: i32,

    /// 입사일 (서버 `joindt`, "YYYY-MM-DD"). 표시용 그대로 보관.
    pub join_date: Option<String>,
    /// 가입일 (서버 `regdt`).
    pub registered_date: Option<String>,
    /// 생일 (서버 `birth`).
    pub birth_date: Option<String>,

    /// 업데이트 권고 여부 (서버 `update == 1`) + 메시지.
    pub update_recommended: bool,
    pub update_message: Option<String>,
}

impl User {
    /// PC 시간 추적 권한. 현재 응답에 명시 필드가 없으므로 기본 true.
    /// TODO(서버 연동): 회사/요금제 정책이 합의되면 그 값으로 교체.
    pub fn can_track_time(&self) -> bool {
        true
    }

    /// UI 에 표시할 이름. `display_name` 비어있으면 이메일 로컬 파트로 fallback.
    pub fn name_for_display(&self) -> String {
        if let Some(n) = self.display_name.as_ref() {
            return n.clone();
        }
        self.email.split('@').next().unwrap_or(&self.email).to_string()
    }
}
