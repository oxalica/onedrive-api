use crate::resource::ErrorObject;
use failure::{format_err, Fail};
use http::StatusCode;
use std::fmt;

/// An alias to `Result` of [`Error`][error].
///
/// [error]: ./struct.Error.html
pub type Result<T> = std::result::Result<T, Error>;

/// Error of API request
#[derive(Debug, Fail)]
pub struct Error {
    #[fail(display = "{}", inner)]
    inner: Box<ErrorKind>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

#[derive(Debug, Fail)]
enum ErrorKind {
    #[fail(display = "Deserialize error: {}", 0)]
    DeserializeError(failure::Error),
    #[fail(display = "Api request failed with {}: {:?}", status, error)]
    ErrorResponse {
        status: StatusCode,
        error: ErrorObject,
    },
    // Errors about ser/de are included.
    #[fail(display = "Request error: {}", 0)]
    RequestError(reqwest::Error),
}

impl Error {
    pub(crate) fn from_error_response(status: StatusCode, error: ErrorObject) -> Self {
        Self {
            inner: Box::new(ErrorKind::ErrorResponse { status, error }),
        }
    }

    pub(crate) fn unexpected_response(reason: &'static str) -> Self {
        Self {
            inner: Box::new(ErrorKind::DeserializeError(format_err!("{}", reason))),
        }
    }

    /// Get the error response from API if caused by error status code.
    pub fn error_response(&self) -> Option<&ErrorObject> {
        match &*self.inner {
            ErrorKind::ErrorResponse { error, .. } => Some(error),
            ErrorKind::DeserializeError(_) | ErrorKind::RequestError(_) => None,
        }
    }

    /// Get the HTTP status code if caused by error status code.
    pub fn status_code(&self) -> Option<StatusCode> {
        match &*self.inner {
            ErrorKind::DeserializeError(_) => None,
            ErrorKind::ErrorResponse { status, .. } => Some(*status),
            ErrorKind::RequestError(source) => source.status(),
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
