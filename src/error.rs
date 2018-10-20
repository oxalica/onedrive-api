use reqwest::Error as ExternRequestError;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub source: Option<Box<dyn StdError>>,
    pub response: Option<String>,
}

#[derive(Debug)]
pub enum ErrorKind {
    /// HTTP client error (4xx). Caused by invalid request.
    /// See `response` field of `Error` for detail.
    ClientError,

    /// HTTP server error (5xx). Caused by server invernal error.
    ServerError,

    /// Other error from `reqwest` or serializiation.
    /// Including network error, invalid response format, etc.
    RequestError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

impl StdError for Error {
    // TODO: fn source(&self) -> Option<&(dyn StdError + 'static)>;
}

impl From<ExternRequestError> for Error {
    fn from(e: ExternRequestError) -> Self {
        use self::ErrorKind::*;

        let kind = if e.is_client_error() {
            ClientError
        } else if e.is_server_error() {
            ServerError
        } else {
            RequestError
        };
        Error {
            kind,
            source: Some(Box::new(e)),
            response: None,
        }
    }
}
