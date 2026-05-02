use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("external command failed: {0}")]
    ExternalCommand(String),
    #[error("application path unavailable: {0}")]
    Path(String),
    #[error("llm disabled: {0}")]
    LlmDisabled(String),
    #[error("llm provider error: {0}")]
    LlmProvider(String),
    #[error("llm schema rejected: {0}")]
    LlmRejected(String),
    #[error("secret storage error: {0}")]
    Secret(String),
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
enum AppErrorDto {
    Database(String),
    Io(String),
    Json(String),
    InvalidInput(String),
    ExternalCommand(String),
    Path(String),
    LlmDisabled(String),
    LlmProvider(String),
    LlmRejected(String),
    Secret(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let dto = match self {
            AppError::Database(message) => AppErrorDto::Database(message.to_string()),
            AppError::Io(message) => AppErrorDto::Io(message.to_string()),
            AppError::Json(message) => AppErrorDto::Json(message.to_string()),
            AppError::InvalidInput(message) => AppErrorDto::InvalidInput(message.clone()),
            AppError::ExternalCommand(message) => AppErrorDto::ExternalCommand(message.clone()),
            AppError::Path(message) => AppErrorDto::Path(message.clone()),
            AppError::LlmDisabled(message) => AppErrorDto::LlmDisabled(message.clone()),
            AppError::LlmProvider(message) => AppErrorDto::LlmProvider(message.clone()),
            AppError::LlmRejected(message) => AppErrorDto::LlmRejected(message.clone()),
            AppError::Secret(message) => AppErrorDto::Secret(message.clone()),
        };
        dto.serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
