use crate::resource::ErrorObject;
use http::StatusCode;
use thiserror::Error;

/// An alias to `Result` of [`Error`][error].
///
/// [error]: ./struct.Error.html
pub type Result<T> = std::result::Result<T, Error>;

/// Error of API request
#[derive(Debug, Error)]
#[error(transparent)]
pub struct Error {
    inner: Box<ErrorKind>,
}

#[derive(Debug, Error)]
enum ErrorKind {
    // Errors about ser/de are included.
    #[error("Request error: {0}")]
    RequestError(reqwest::Error),
    #[error("Unexpected response: {reason}")]
    UnexpectedResponse { reason: &'static str },
    #[error(
        "Api request failed with {status}: ({}) {}",
        .response.code.as_deref().unwrap_or_default(),
        .response.message.as_deref().unwrap_or_default(),
    )]
    ErrorResponse {
        status: StatusCode,
        response: ErrorObject,
    },
}

impl Error {
    pub(crate) fn from_error_response(status: StatusCode, response: ErrorObject) -> Self {
        Self {
            inner: Box::new(ErrorKind::ErrorResponse { status, response }),
        }
    }

    pub(crate) fn unexpected_response(reason: &'static str) -> Self {
        Self {
            inner: Box::new(ErrorKind::UnexpectedResponse { reason }),
        }
    }

    /// Get the error response from API if caused by error status code.
    pub fn error_response(&self) -> Option<&ErrorObject> {
        match &*self.inner {
            ErrorKind::ErrorResponse { response, .. } => Some(response),
            _ => None,
        }
    }

    /// Get the HTTP status code if caused by error status code.
    pub fn status_code(&self) -> Option<StatusCode> {
        match &*self.inner {
            ErrorKind::RequestError(source) => source.status(),
            ErrorKind::UnexpectedResponse { .. } => None,
            ErrorKind::ErrorResponse { status, .. } => Some(*status),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(source: reqwest::Error) -> Self {
        Self {
            inner: Box::new(ErrorKind::RequestError(source)),
        }
    }
}
