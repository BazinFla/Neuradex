use thiserror::Error;

/// Structured, strongly-typed error types for API operations in NeuraDex.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network connection error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("HTTP error {status}: {message}")]
    HttpStatus {
        status: u16,
        message: String,
    },

    #[error("Serialization / JSON format error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Custom(String),
}

impl From<String> for ApiError {
    fn from(s: String) -> Self {
        ApiError::Custom(s)
    }
}

impl From<&str> for ApiError {
    fn from(s: &str) -> Self {
        ApiError::Custom(s.to_string())
    }
}
