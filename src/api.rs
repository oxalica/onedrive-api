use crate::error::Result;
use http;

pub(crate) type RawRequest = http::Request<Vec<u8>>;
pub(crate) type RawResponse = http::Response<Vec<u8>>;

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// An abstract API request, conbining constructed HTTP request and response parser.
///
/// The operation of API will **only** be performed when it is [`execute`][execute]d
/// by an HTTP [`Client`][client].
///
/// [execute]: #method.execute
/// [client]: trait.Client.html
#[must_use = "Api do nothing unless you `execute` them"]
pub trait Api: sealed::Sealed + Send + Sync + Sized {
    /// The response of this API endpoint.
    type Response;

    /// Perform the operation through an HTTP [`Client`][client].
    ///
    /// Note that some `Api` may execute zero or more than one requests.
    /// See the documentation of the api function for more detail.
    ///
    /// [execute]: #method.execute
    fn execute(self, client: &impl Client) -> Result<Self::Response>;
}

pub(crate) struct SimpleApi {
    request: Result<RawRequest>,
}

impl sealed::Sealed for SimpleApi {}

impl Api for SimpleApi {
    type Response = RawResponse;

    fn execute(self, client: &impl Client) -> Result<Self::Response> {
        client.execute_api(self.request?)
    }
}

impl SimpleApi {
    pub(crate) fn new(request: Result<RawRequest>) -> Self {
        Self { request }
    }
}

pub(crate) trait ApiExt: Api {
    fn and_then<T, F>(self, f: F) -> AndThen<Self, F>
    where
        F: FnOnce(Self::Response) -> Result<T> + Send + Sync,
    {
        AndThen { api: self, f }
    }
}

impl<A: Api> ApiExt for A {}

pub(crate) struct AndThen<A, F> {
    api: A,
    f: F,
}

impl<A, F> sealed::Sealed for AndThen<A, F> {}

impl<A: Api, T, F> Api for AndThen<A, F>
where
    F: FnOnce(A::Response) -> Result<T> + Send + Sync,
{
    type Response = T;

    fn execute(self, client: &impl Client) -> Result<Self::Response> {
        self.api.execute(client).and_then(self.f)
    }
}

/// Abstract synchronous HTTP client
pub trait Client {
    /// Perform the operation of this API and get the result.
    fn execute_api(&self, req: RawRequest) -> Result<RawResponse>;
}

// FIXME: Avoid copy
#[cfg(feature = "reqwest")]
fn to_reqwest_request(req: RawRequest) -> ::reqwest::Request {
    let (parts, body) = req.into_parts();
    let url = ::reqwest::Url::parse(&parts.uri.to_string()).unwrap();
    let mut req = ::reqwest::Request::new(parts.method, url);
    *req.headers_mut() = parts.headers;
    *req.body_mut() = Some(body.into());
    req
}

// FIXME: Avoid copy
#[cfg(feature = "reqwest")]
fn from_reqwest_response(mut resp: ::reqwest::Response) -> Result<RawResponse> {
    let mut b = http::Response::builder();
    b.headers_mut().unwrap().clone_from(resp.headers());
    let mut buf = vec![];
    resp.copy_to(&mut buf)?;
    Ok(b.status(resp.status()).body(buf)?)
}

#[cfg(feature = "reqwest")]
impl Client for ::reqwest::Client {
    fn execute_api(&self, req: RawRequest) -> Result<RawResponse> {
        let req = to_reqwest_request(req);
        let resp = self.execute(req)?;
        from_reqwest_response(resp)
    }
}
