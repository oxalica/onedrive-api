use self::ErrorKind::*;
use crate::resource::ErrorObject;
use reqwest::StatusCode;
use std::fmt;
use std::result::Result as StdResult;

pub type Result<T> = StdResult<T, Error>;

/// The error may occur when processing requests.
#[derive(Debug)]
pub struct Error {
    // Make the size of `Error` smaller.
    inner: Box<InnerError>,
}

#[derive(Debug)]
struct InnerError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    RequestError {
        source: reqwest::Error,
        response: Option<ErrorObject>,
    },
    UnexpectedResponse {
        reason: &'static str,
    },
}

impl Error {
    pub(crate) fn unexpected_response(reason: &'static str) -> Self {
        Self {
            inner: Box::new(InnerError {
                kind: ErrorKind::UnexpectedResponse { reason },
            }),
        }
    }

    pub(crate) fn from_response(source: reqwest::Error, response: Option<ErrorObject>) -> Self {
        Self {
            inner: Box::new(InnerError {
                kind: ErrorKind::RequestError { source, response },
            }),
        }
    }

    /// Check whether the error may be recovered by retrying.
    pub fn should_retry(&self) -> bool {
        match &self.inner.kind {
            RequestError { source, .. } => !source.is_client_error() && !source.is_serialization(),
            _ => false,
        }
    }

    /// Get the url related to the error.
    pub fn url(&self) -> Option<&reqwest::Url> {
        match &self.inner.kind {
            RequestError { source, .. } => source.url(),
            _ => None,
        }
    }

    /// Get the error response from API if caused by error status code.
    pub fn error_response(&self) -> Option<&ErrorObject> {
        match &self.inner.kind {
            RequestError { response, .. } => response.as_ref(),
            _ => None,
        }
    }

    /// Get the HTTP status code if caused by error status code.
    pub fn status_code(&self) -> Option<StatusCode> {
        match &self.inner.kind {
            RequestError { source, .. } => source.status(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.inner.kind {
            RequestError { source, .. } => write!(f, "{}", source),
            UnexpectedResponse { reason } => write!(f, "{}", reason),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner.kind {
            RequestError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(source: reqwest::Error) -> Self {
        Self::from_response(source, None)
    }
}
