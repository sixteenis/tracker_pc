//! 회사 정보 도메인 모델.

#[derive(Debug, Clone)]
pub struct Company {
    pub id: i64,
    pub name: String,
}
