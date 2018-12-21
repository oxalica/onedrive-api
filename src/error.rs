use self::Error::*;
use reqwest::StatusCode;
use std::fmt;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    UnexpectedResponse {
        reason: &'static str,
    },
    RequestError {
        source: reqwest::Error,
        response: Option<String>,
    },
}

impl Error {
    pub fn should_retry(&self) -> bool {
        match self {
            RequestError { source, .. } => !source.is_client_error() && !source.is_serialization(),
            _ => false,
        }
    }

    pub fn response(&self) -> Option<&str> {
        match self {
            RequestError { response, .. } => response.as_ref().map(|s| &**s),
            _ => None,
        }
    }

    pub fn status(&self) -> Option<StatusCode> {
        match self {
            RequestError { source, .. } => source.status(),
            UnexpectedResponse { .. } => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UnexpectedResponse { reason } => write!(f, "{}", reason),
            RequestError { source, .. } => write!(f, "{}", source),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RequestError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(source: reqwest::Error) -> Self {
        RequestError {
            source,
            response: None,
        }
    }
}
