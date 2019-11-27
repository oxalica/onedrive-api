use crate::resource::ErrorObject;
use failure::{self, format_err, Fail};
use http::{self, StatusCode};
use serde_json;
use std::fmt;

/// An alias to `Result` of [`Error`][error].
///
/// [error]: ./struct.Error.html
pub type Result<T> = std::result::Result<T, Error>;

/// Error of API request
#[derive(Debug)]
pub struct Error {
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
    #[fail(display = "HTTP error: {}", 0)]
    HttpError(http::Error),
    #[fail(display = "HTTP error: {}", 0)]
    UrlParseError(url::ParseError),
    #[fail(display = "Api request failed with {}: {:?}", status, error)]
    ErrorResponse {
        status: StatusCode,
        error: ErrorObject,
    },
    #[cfg(feature = "reqwest")]
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
            ErrorKind::HttpError(_)
            | ErrorKind::DeserializeError(_)
            | ErrorKind::UrlParseError(_) => None,
            ErrorKind::ErrorResponse { error, .. } => Some(error),
            #[cfg(feature = "reqwest")]
            ErrorKind::RequestError(_) => None,
        }
    }

    /// Get the HTTP status code if caused by error status code.
    pub fn status_code(&self) -> Option<StatusCode> {
        match &*self.inner {
            ErrorKind::DeserializeError(_)
            | ErrorKind::HttpError(_)
            | ErrorKind::UrlParseError(_) => None,
            ErrorKind::ErrorResponse { status, .. } => Some(*status),
            #[cfg(feature = "reqwest")]
            ErrorKind::RequestError(source) => source.status(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self {
            inner: Box::new(ErrorKind::DeserializeError(e.into())),
        }
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Self {
            inner: Box::new(ErrorKind::HttpError(e.into())),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(source: url::ParseError) -> Self {
        Self {
            inner: Box::new(ErrorKind::UrlParseError(source)),
        }
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for Error {
    fn from(source: reqwest::Error) -> Self {
        Self {
            inner: Box::new(ErrorKind::RequestError(source)),
        }
    }
}
