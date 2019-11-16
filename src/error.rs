use crate::resource::ErrorObject;
use failure::{self, format_err, Fail};
use http::{self, StatusCode};
use serde_json;
use serde_urlencoded;
use std::fmt;

/// An alias to `Result` with `Err` of `onedrive_api::Error`.
pub type Result<T> = std::result::Result<T, Error>;

/// The error may occur when processing requests.
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
    #[fail(display = "Serialize error: {}", 0)]
    SerializeError(failure::Error),
    #[fail(display = "Deserialize error: {}", 0)]
    DeserializeError(failure::Error),
    #[fail(display = "HTTP error: {}", 0)]
    HttpError(http::Error),
    #[fail(display = "Api request failed with {}: {:?}", status, error)]
    ErrorResponse {
        status: StatusCode,
        error: ErrorObject,
    },
    #[fail(display = "Request error: {} (response: {:?})", source, response)]
    RequestError {
        source: reqwest::Error,
        response: Option<ErrorObject>,
    },
}

impl Error {
    // TODO: Remove this
    pub(crate) fn from_response(source: reqwest::Error, response: Option<ErrorObject>) -> Self {
        Self {
            inner: Box::new(ErrorKind::RequestError { source, response }),
        }
    }

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

    /// Check whether the error may be recovered by retrying.
    // TODO: Remove this
    pub fn should_retry(&self) -> bool {
        match &*self.inner {
            ErrorKind::RequestError { source, .. } => {
                !source.is_client_error() && !source.is_serialization()
            }
            _ => false,
        }
    }

    /// Get the url related to the error.
    // TODO: Remove this
    pub fn url(&self) -> Option<&reqwest::Url> {
        match &*self.inner {
            ErrorKind::RequestError { source, .. } => source.url(),
            _ => None,
        }
    }

    /// Get the error response from API if caused by error status code.
    pub fn error_response(&self) -> Option<&ErrorObject> {
        match &*self.inner {
            ErrorKind::SerializeError(_)
            | ErrorKind::HttpError(_)
            | ErrorKind::DeserializeError(_) => None,
            ErrorKind::ErrorResponse { error, .. } => Some(error),
            ErrorKind::RequestError { response, .. } => response.as_ref(),
        }
    }

    /// Get the HTTP status code if caused by error status code.
    pub fn status_code(&self) -> Option<StatusCode> {
        match &*self.inner {
            ErrorKind::SerializeError(_)
            | ErrorKind::DeserializeError(_)
            | ErrorKind::HttpError(_) => None,
            ErrorKind::ErrorResponse { status, .. } => Some(*status),
            ErrorKind::RequestError { source, .. } => source.status(),
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

impl From<serde_urlencoded::ser::Error> for Error {
    fn from(e: serde_urlencoded::ser::Error) -> Self {
        Self {
            inner: Box::new(ErrorKind::SerializeError(e.into())),
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

impl From<reqwest::Error> for Error {
    fn from(source: reqwest::Error) -> Self {
        Self::from_response(source, None)
    }
}
