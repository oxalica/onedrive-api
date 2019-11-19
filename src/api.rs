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

    /// Get the constructed HTTP request of this API.
    ///
    /// At most time, you should call [`execute`][execute] instead of calling this directly.
    ///
    /// # Panic
    /// May panic if called twice.
    ///
    /// [execute]: #method.execute
    fn get_request(&mut self) -> Result<RawRequest>;

    /// Parse the raw response.
    ///
    /// At most time, you should call [`execute`][execute] instead of calling this directly.
    ///
    /// [execute]: #method.execute
    fn parse(self, resp: RawResponse) -> Result<Self::Response>;

    /// Perform the operation through an HTTP [`Client`][client].
    ///
    /// [execute]: #method.execute
    fn execute(self, client: &impl Client) -> Result<Self::Response> {
        client.execute_api(self)
    }
}

pub(crate) struct TrivialApi {
    request: Option<Result<RawRequest>>,
}

impl sealed::Sealed for TrivialApi {}

impl Api for TrivialApi {
    type Response = RawResponse;

    fn get_request(&mut self) -> Result<RawRequest> {
        self.request.take().unwrap()
    }

    fn parse(self, resp: RawResponse) -> Result<Self::Response> {
        Ok(resp)
    }
}

impl TrivialApi {
    pub(crate) fn new(req: Result<RawRequest>) -> Self {
        Self { request: Some(req) }
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

    fn get_request(&mut self) -> Result<RawRequest> {
        self.api.get_request()
    }

    fn parse(self, resp: RawResponse) -> Result<Self::Response> {
        (self.f)(self.api.parse(resp)?)
    }
}

/// Abstract synchronous HTTP client
pub trait Client {
    /// Perform the operation of this API and get the result.
    fn execute_api<A: Api>(&self, api: A) -> Result<A::Response>;
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
    fn execute_api<A: Api>(&self, mut api: A) -> Result<A::Response> {
        let req = to_reqwest_request(api.get_request()?);
        let resp = self.execute(req)?;
        let resp = from_reqwest_response(resp)?;
        api.parse(resp)
    }
}
