//! 회사 정보 repository — 현재 mock.
//!
//! 로그인 응답에 `cmpname` 이 같이 들어오므로 보통은 `auth_repository::login`
//! 시점에 `company_service::set` 으로 캐싱된 값을 사용한다. 본 mock 은
//! 별도 회사 상세 조회 API 합의 전 임시 fallback.

use crate::domain::model::company::Company;

pub fn mock_company() -> Company {
    Company {
        id: 11402,
        name: "성민 (Mock)".to_string(),
    }
}
