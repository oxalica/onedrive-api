use std::time::Duration;

use crate::resource::{ErrorResponse, OAuth2ErrorResponse};
use reqwest::StatusCode;
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
    RequestError(#[source] reqwest::Error),
    #[error("Unexpected response: {reason}")]
    UnexpectedResponse { reason: &'static str },
    #[error("Api error with {status}: ({}) {}", .response.code, .response.message)]
    ErrorResponse {
        status: StatusCode,
        response: ErrorResponse,
        retry_after: Option<u32>,
    },
    #[error("OAuth2 error with {status}: ({}) {}", .response.error, .response.error_description)]
    OAuth2Error {
        status: StatusCode,
        response: OAuth2ErrorResponse,
        retry_after: Option<u32>,
    },
}

impl Error {
    pub(crate) fn from_error_response(
        status: StatusCode,
        response: ErrorResponse,
        retry_after: Option<u32>,
    ) -> Self {
        Self {
            inner: Box::new(ErrorKind::ErrorResponse {
                status,
                response,
                retry_after,
            }),
        }
    }

    pub(crate) fn unexpected_response(reason: &'static str) -> Self {
        Self {
            inner: Box::new(ErrorKind::UnexpectedResponse { reason }),
        }
    }

    pub(crate) fn from_oauth2_error_response(
        status: StatusCode,
        response: OAuth2ErrorResponse,
        retry_after: Option<u32>,
    ) -> Self {
        Self {
            inner: Box::new(ErrorKind::OAuth2Error {
                status,
                response,
                retry_after,
            }),
        }
    }

    /// Get the error response from API if caused by error status code.
    #[must_use]
    pub fn error_response(&self) -> Option<&ErrorResponse> {
        match &*self.inner {
            ErrorKind::ErrorResponse { response, .. } => Some(response),
            _ => None,
        }
    }

    /// Get the OAuth2 error response from API if caused by OAuth2 error response.
    #[must_use]
    pub fn oauth2_error_response(&self) -> Option<&OAuth2ErrorResponse> {
        match &*self.inner {
            ErrorKind::OAuth2Error { response, .. } => Some(response),
            _ => None,
        }
    }

    /// Get the HTTP status code if caused by error status code.
    #[must_use]
    pub fn status_code(&self) -> Option<StatusCode> {
        match &*self.inner {
            ErrorKind::RequestError(source) => source.status(),
            ErrorKind::UnexpectedResponse { .. } => None,
            ErrorKind::ErrorResponse { status, .. } | ErrorKind::OAuth2Error { status, .. } => {
                Some(*status)
            }
        }
    }

    /// Get the retry delay hint on rate limited (HTTP 429) or server unavailability, if any.
    ///
    /// This is parsed from response header `Retry-After`.
    /// See: <https://learn.microsoft.com/en-us/graph/throttling>
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match &*self.inner {
            ErrorKind::ErrorResponse { retry_after, .. }
            | ErrorKind::OAuth2Error { retry_after, .. } => {
                Some(Duration::from_secs((*retry_after)?.into()))
            }
            _ => None,
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

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn error_source() {
        let err = reqwest::blocking::get("urn:urn").unwrap_err();
        let original_err_fmt = err.to_string();
        let source_err_fmt = Error::from(err).source().unwrap().to_string();
        assert_eq!(source_err_fmt, original_err_fmt);
    }
}
