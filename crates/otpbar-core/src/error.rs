use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("금고가 잠겨 있습니다")]
    Locked,

    #[error("마스터 암호가 올바르지 않습니다")]
    BadPassword,

    #[error("금고를 아직 만들지 않았습니다")]
    NoVault,

    #[error("금고가 이미 있습니다")]
    VaultExists,

    #[error("계정을 찾지 못했습니다: {0}")]
    AccountNotFound(String),

    #[error("계정이 여러 개 일치합니다: {0}")]
    Ambiguous(String),

    /// 같은 발급자에 같은 이름의 계정이 이미 있다. 값은 표시용 이름(`발급자 · 이름`).
    #[error("같은 이름의 계정이 이미 있습니다: {0}")]
    DuplicateName(String),

    /// 계정 이름으로 쓸 수 없는 값(빈 이름, 너무 긴 이름).
    #[error("{0}")]
    InvalidName(String),

    #[error("비밀키 형식 오류: {0}")]
    InvalidSecret(String),

    #[error("값 범위 오류: {0}")]
    InvalidParams(String),

    #[error("지원하지 않음: {0}")]
    Unsupported(String),

    #[error("QR 인식 실패: {0}")]
    Qr(String),

    #[error("금고 파일이 손상되었습니다: {0}")]
    Corrupt(String),

    #[error("입출력 오류: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 오류: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}
