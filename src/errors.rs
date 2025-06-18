use thiserror::Error;

#[derive(Error, Debug)]
pub enum SingleloadError {
    #[error("Container error: {0}")]
    Container(String),

    #[error("Execution timeout exceeded")]
    Timeout,

    #[error("Output size limit exceeded")]
    OutputLimitExceeded,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Podman API error: {0}")]
    PodmanApi(#[from] podman_api::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Base image not found. Run 'singleload install' first")]
    BaseImageNotFound,

    #[error("Script file not found: {0}")]
    ScriptNotFound(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Container escape attempt detected")]
    ContainerEscape,

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}