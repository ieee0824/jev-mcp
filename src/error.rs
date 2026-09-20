use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Validation,
    Authentication,
    RateLimit,
    Timeout,
    Network,
    Http,
    InvalidResponse,
}

#[derive(Debug, Serialize)]
pub struct ToolError {
    pub kind: ErrorKind,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

impl ToolError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message, false, None)
    }

    pub fn validation_response(message: impl Into<String>, status: u16) -> Self {
        Self::new(ErrorKind::Validation, message, false, Some(status))
    }

    pub fn authentication(message: impl Into<String>, status: Option<u16>) -> Self {
        Self::new(ErrorKind::Authentication, message, false, status)
    }

    pub fn rate_limit(message: impl Into<String>, status: u16) -> Self {
        Self::new(ErrorKind::RateLimit, message, true, Some(status))
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message, true, None)
    }

    pub fn network(message: impl Into<String>, status: Option<u16>) -> Self {
        Self::new(ErrorKind::Network, message, true, status)
    }

    pub fn http(message: impl Into<String>, status: u16, retryable: bool) -> Self {
        Self::new(ErrorKind::Http, message, retryable, Some(status))
    }

    pub fn invalid_response(message: impl Into<String>, status: u16) -> Self {
        Self::new(ErrorKind::InvalidResponse, message, false, Some(status))
    }

    fn new(
        kind: ErrorKind,
        message: impl Into<String>,
        retryable: bool,
        status: Option<u16>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable,
            status,
        }
    }
}
