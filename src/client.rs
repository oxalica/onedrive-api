use super::error::*;
use super::resource::*;
use reqwest::{header, Client as RequestClient, RequestBuilder, Response, StatusCode};
use serde::de;
use std::ops::Range;
use url::{PathSegmentsMut, Url};

/// Scopes determine what type of access the app is granted when the user is signed in.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#authentication-scopes
#[derive(Clone, Debug)]
pub enum Scope {
    Read { shared: bool, offline: bool },
    ReadWrite { shared: bool, offline: bool },
}

impl Scope {
    pub fn shared(&self) -> bool {
        match self {
            Scope::Read { shared, .. } | Scope::ReadWrite { shared, .. } => *shared,
        }
    }

    pub fn offline(&self) -> bool {
        match self {
            Scope::Read { offline, .. } | Scope::ReadWrite { offline, .. } => *offline,
        }
    }

    fn to_scope_string(&self) -> &'static str {
        use self::Scope::*;

        let mut s = match self {
            Read { .. } => "offline_access files.read.all",
            ReadWrite { .. } => "offline_access files.readwrite.all",
        };

        match self {
            Read { offline, shared } | ReadWrite { offline, shared } => {
                if !offline {
                    s = &s["offline_access ".len()..];
                }
                if !shared {
                    s = &s[..s.len() - ".all".len()];
                }
            }
        }

        s
    }
}

/// Specify a `Drive` resource.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/drive_get?view=odsp-graph-online
#[derive(Clone, Debug)]
pub enum DriveLocation<'a> {
    CurrentDrive,
    UserId(&'a str),
    GroupId(&'a str),
    SiteId(&'a str),
    DriveId(&'a str),
}

impl<'a> From<&'a Drive> for DriveLocation<'a> {
    fn from(drive: &'a Drive) -> Self {
        From::from(&drive.id)
    }
}

impl<'a> From<&'a DriveId> for DriveLocation<'a> {
    fn from(id: &'a DriveId) -> Self {
        DriveLocation::DriveId(id.as_ref())
    }
}

/// Specify a `DriveItem` resource.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_get?view=odsp-graph-online
#[derive(Clone, Debug)]
pub enum ItemLocation<'a> {
    ItemId(&'a str),
    Path(&'a str),
}

impl<'a> From<&'a str> for ItemLocation<'a> {
    fn from(path: &'a str) -> Self {
        ItemLocation::Path(path)
    }
}

impl<'a> From<&'a DriveItem> for ItemLocation<'a> {
    fn from(item: &'a DriveItem) -> Self {
        From::from(&item.id)
    }
}

impl<'a> From<&'a ItemId> for ItemLocation<'a> {
    fn from(id: &'a ItemId) -> Self {
        ItemLocation::ItemId(id.as_ref())
    }
}

/// The client for requests relative to authentication.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online
pub struct AuthClient {
    client: RequestClient,
    client_id: String,
    scope: Scope,
    redirect_uri: String,
}

impl AuthClient {
    pub fn new(client_id: String, scope: Scope, redirect_uri: String) -> Self {
        AuthClient {
            client: RequestClient::new(),
            client_id,
            scope,
            redirect_uri,
        }
    }

    fn get_auth_url(&self, response_type: &str) -> String {
        Url::parse_with_params(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            &[
                ("client_id", &self.client_id as &str),
                ("scope", self.scope.to_scope_string()),
                ("redirect_uri", &self.redirect_uri),
                ("response_type", response_type),
            ],
        )
        .unwrap()
        .into_string()
    }

    /// Get the URL for web browser for token flow authentication.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#token-flow
    pub fn get_token_auth_url(&self) -> String {
        self.get_auth_url("token")
    }

    /// Get the URL for web browser for code flow authentication.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-1-get-an-authorization-code
    pub fn get_code_auth_url(&self) -> String {
        self.get_auth_url("code")
    }

    fn request_authorize(&self, require_refresh: bool, params: &[(&str, &str)]) -> Result<Client> {
        #[derive(Deserialize)]
        struct Response {
            // token_type: String,
            // expires_in: u64,
            // scope: String,
            access_token: String,
            refresh_token: Option<String>,
        }

        let resp: Response = self
            .client
            .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            .form(params)
            .send()?
            .parse()?;

        if require_refresh && resp.refresh_token.is_none() {
            // Missing `refresh_token`
            Err(Error {
                kind: ErrorKind::RequestError,
                source: None,
                response: None,
            })?;
        }

        Ok(Client::new(resp.access_token, resp.refresh_token))
    }

    /// Login using a code in code flow authentication.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-2-redeem-the-code-for-access-tokens
    pub fn login_with_code(&self, code: &str, client_secret: Option<&str>) -> Result<Client> {
        self.request_authorize(
            self.scope.offline(),
            &[
                ("client_id", &self.client_id as &str),
                ("client_secret", client_secret.unwrap_or("")),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &self.redirect_uri),
            ],
        )
    }

    /// Login using a refresh token.
    /// This requires offline access.
    ///
    /// # Panic
    /// Panic if the `scope` given in `Client::new` has no `offline_access` scope.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-3-get-a-new-access-token-or-refresh-token
    pub fn login_with_refresh_token(
        &self,
        refresh_token: &str,
        client_secret: Option<&str>,
    ) -> Result<Client> {
        assert!(
            self.scope.offline(),
            "Refresh token requires offline_access scope."
        );

        self.request_authorize(
            true,
            &[
                ("client_id", &self.client_id as &str),
                ("client_secret", client_secret.unwrap_or("")),
                ("grant_type", "refresh_token"),
                ("redirect_uri", &self.redirect_uri),
                ("refresh_token", refresh_token),
            ],
        )
    }
}

/// The client for requests accessing OneDrive resources.
pub struct Client {
    client: RequestClient,
    token: String,
    refresh_token: Option<String>,
}

fn extend_drive_url_parts(segs: &mut PathSegmentsMut, drive: &DriveLocation) {
    use self::DriveLocation::*;
    match drive {
        CurrentDrive => segs.push("drive"),
        UserId(id) => segs.extend(&["users", id, "drive"]),
        GroupId(id) => segs.extend(&["groups", id, "drive"]),
        SiteId(id) => segs.extend(&["sites", id, "drive"]),
        DriveId(id) => segs.extend(&["drives", id, "drive"]),
    };
}

fn extend_item_url_parts(segs: &mut PathSegmentsMut, item: &ItemLocation) {
    use self::ItemLocation::*;
    match item {
        ItemId(id) => segs.extend(&["items", id]),
        Path("/") => segs.push("root"),
        Path(path) => segs.push(&["root:", path, ":"].join("")),
    };
}

macro_rules! api_url {
    (@__impl $segs:expr; $(,)*) => {};
    (@__impl $segs:expr; @drive $e:expr, $($t:tt)*) => {
        extend_drive_url_parts($segs, $e);
        api_url!(@__impl $segs; $($t)*);
    };
    (@__impl $segs:expr; @item $e:expr, $($t:tt)*) => {
        extend_item_url_parts($segs, $e);
        api_url!(@__impl $segs; $($t)*);
    };
    (@__impl $segs:expr; $e:expr, $($t:tt)*) => {
        $segs.push($e);
        api_url!(@__impl $segs; $($t)*)
    };
    ($($t:tt)*) => {
        {
            let mut url = Url::parse("https://graph.microsoft.com/v1.0").unwrap();
            {
                let mut segs = url.path_segments_mut().unwrap();
                api_url!(@__impl &mut segs; $($t)* ,); // Trailing comma for matching
            }
            url
        }
    };
}

impl Client {
    pub fn new(token: String, refresh_token: Option<String>) -> Self {
        Client {
            client: RequestClient::new(),
            token,
            refresh_token,
        }
    }

    pub fn get_token(&self) -> &str {
        &self.token
    }

    pub fn get_refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_ref().map(|s| &**s)
    }

    /// Get the `Drive` resource.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/drive_get?view=odsp-graph-online
    pub fn get_drive<'a>(&self, drive: impl Into<DriveLocation<'a>>) -> Result<Drive> {
        self.client
            .get(api_url![@drive &drive.into()])
            .bearer_auth(&self.token)
            .send()?
            .parse()
    }

    /// List children of a `DriveItem` resource.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_list_children?view=odsp-graph-online
    pub fn list_children<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        none_if_match: Option<&Tag>,
    ) -> Result<Option<Vec<DriveItem>>> {
        #[derive(Deserialize)]
        struct Response {
            value: Vec<DriveItem>,
            #[serde(rename = "@odata.nextLink")]
            next_link: Option<String>,
        }

        let fetch = |url: &str, tag: Option<&Tag>| -> Result<Option<Response>> {
            self.client
                .get(url)
                .bearer_auth(&self.token)
                .opt_header("if-none-match", tag)
                .send()?
                .parse_or_none(StatusCode::NOT_MODIFIED)
        };

        let url = api_url![
            @drive &drive.into(),
            @item &item.into(),
            "children",
        ];
        match fetch(url.as_ref(), none_if_match)? {
            None => Ok(None),
            Some(Response {
                mut value,
                mut next_link,
            }) => {
                while let Some(link) = next_link {
                    let resp = fetch(&link, None)?.unwrap(); // No `match_Tag`
                    value.extend(resp.value);
                    next_link = resp.next_link;
                }
                Ok(Some(value))
            }
        }
    }

    /// Get a `DriveItem` resource.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_get?view=odsp-graph-online
    pub fn get_item<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        none_if_match: Option<&Tag>,
    ) -> Result<Option<DriveItem>> {
        self.client
            .get(api_url![
                @drive &drive.into(),
                @item &item.into(),
            ])
            .bearer_auth(&self.token)
            .opt_header("if-none-match", none_if_match)
            .send()?
            .parse_or_none(StatusCode::NOT_MODIFIED)
    }

    const SMALL_FILE_SIZE: usize = 4 << 20; // 4 MB

    /// Upload a small file.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content?view=odsp-graph-online
    pub fn upload_small<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        data: &[u8],
    ) -> Result<DriveItem> {
        assert!(
            data.len() <= Self::SMALL_FILE_SIZE,
            "Uploading large file requires upload session"
        );

        self.client
            .put(api_url![
                @drive &drive.into(),
                @item &item.into(),
                "content",
            ])
            .bearer_auth(&self.token)
            .body(data.to_owned())
            .send()?
            .parse()
    }

    pub fn new_upload_session<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        overwrite: bool,
        none_if_match: Option<&Tag>,
    ) -> Result<Option<UploadSession>> {
        #[derive(Serialize)]
        struct Item {
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: &'static str,
        }

        #[derive(Serialize)]
        struct Request {
            item: Item,
        }

        self.client
            .post(api_url![
                @drive &drive.into(),
                @item &item.into(),
                "createUploadSession",
            ])
            .opt_header("if-match", none_if_match)
            .bearer_auth(&self.token)
            .json(&Request {
                item: Item {
                    conflict_behavior: if overwrite { "overwrite" } else { "fail" },
                },
            })
            .send()?
            .parse_or_none(StatusCode::PRECONDITION_FAILED)
    }

    pub fn get_upload_session(&self, upload_url: &str) -> Result<UploadSession> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct UploadSessionResponse {
            upload_url: Option<String>,
            next_expected_ranges: Vec<ExpectRange>,
            // expiration_date_time: Timestamp,
        }

        let resp = self
            .client
            .get(upload_url)
            .send()?
            .parse::<UploadSessionResponse>()?;

        Ok(UploadSession {
            upload_url: resp.upload_url.unwrap_or_else(|| upload_url.to_owned()),
            next_expected_ranges: resp.next_expected_ranges,
        })
    }

    pub fn delete_upload_session(&self, sess: &UploadSession) -> Result<()> {
        self.client
            .delete(&sess.upload_url)
            .send()?
            .check_status()?;
        Ok(())
    }

    const SESSION_UPLOAD_MAX_FILE_SIZE: usize = 60 << 20; // 60 MiB

    pub fn upload_to_session(
        &self,
        session: &UploadSession,
        data: &[u8],
        remote_range: Range<usize>,
        total_size: usize,
    ) -> Result<Option<DriveItem>> {
        assert!(
            remote_range.start <= remote_range.end && remote_range.end <= total_size,
            "Invalid range",
        );
        assert_eq!(
            data.len(),
            remote_range.end - remote_range.start,
            "Length mismatch"
        );
        assert!(
            data.len() < Self::SESSION_UPLOAD_MAX_FILE_SIZE,
            "Data too long"
        );

        self.client
            .put(&session.upload_url)
            // No auth token
            .header(
                header::CONTENT_RANGE,
                format!(
                    "bytes {}-{}/{}",
                    remote_range.start,
                    remote_range.end - 1,
                    total_size
                ),
            )
            .body(data.to_owned())
            .send()?
            .parse_or_none(StatusCode::ACCEPTED)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadSession {
    upload_url: String,
    next_expected_ranges: Vec<ExpectRange>,
    // expiration_date_time: Timestamp,
}

impl UploadSession {
    pub fn get_url(&self) -> &str {
        &self.upload_url
    }

    pub fn get_next_expected_ranges(&self) -> &[ExpectRange] {
        &self.next_expected_ranges
    }
}

#[derive(Debug)]
pub struct ExpectRange {
    pub start: usize,
    pub end: Option<usize>,
}

impl<'de> de::Deserialize<'de> for ExpectRange {
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> ::std::result::Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = ExpectRange;

            fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "Expect Range")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> ::std::result::Result<Self::Value, E> {
                let parse = || -> Option<ExpectRange> {
                    let mut it = v.split('-');
                    let l = it.next()?;
                    let r = it.next()?;
                    if it.next().is_some() {
                        return None;
                    }
                    Some(ExpectRange {
                        start: l.parse().ok()?,
                        end: if r.is_empty() {
                            None
                        } else {
                            Some(r.parse::<usize>().ok()?.checked_add(1)?)
                        },
                    })
                };
                match parse() {
                    Some(v) => Ok(v),
                    None => Err(E::invalid_value(
                        de::Unexpected::Str(v),
                        &"`{usize}-` or `{usize}-{usize}`",
                    )),
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

trait RequestBuilderExt: Sized {
    fn opt_header(self, key: impl AsRef<str>, value: Option<impl AsRef<str>>) -> Self;
}

impl RequestBuilderExt for RequestBuilder {
    fn opt_header(self, key: impl AsRef<str>, value: Option<impl AsRef<str>>) -> Self {
        match value {
            Some(v) => self.header(key.as_ref(), v.as_ref()),
            None => self,
        }
    }
}

trait ResponseExt: Sized {
    fn check_status(self) -> Result<Self>;
    fn parse<T: de::DeserializeOwned>(self) -> Result<T>;
    /// Allow `status` as None. For code like `304 Not Modified`.
    fn parse_or_none<T: de::DeserializeOwned>(self, status: StatusCode) -> Result<Option<T>>;
}

impl ResponseExt for Response {
    fn check_status(mut self) -> Result<Self> {
        if self.status().is_success() {
            Ok(self)
        } else {
            let resp = self.text()?;
            let mut e = Error::from(self.error_for_status().unwrap_err());
            e.response = Some(resp);
            Err(e)
        }
    }

    fn parse<T: de::DeserializeOwned>(self) -> Result<T> {
        Ok(self.check_status()?.json()?)
    }

    fn parse_or_none<T: de::DeserializeOwned>(self, status: StatusCode) -> Result<Option<T>> {
        if self.status() == status {
            Ok(None)
        } else {
            self.parse().map(Option::Some)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_scope_string() {
        use self::Scope::*;

        let scopes = [
            (
                Read {
                    offline: false,
                    shared: false,
                },
                "files.read",
            ),
            (
                Read {
                    offline: false,
                    shared: true,
                },
                "files.read.all",
            ),
            (
                Read {
                    offline: true,
                    shared: false,
                },
                "offline_access files.read",
            ),
            (
                Read {
                    offline: true,
                    shared: true,
                },
                "offline_access files.read.all",
            ),
            (
                ReadWrite {
                    offline: false,
                    shared: false,
                },
                "files.readwrite",
            ),
            (
                ReadWrite {
                    offline: false,
                    shared: true,
                },
                "files.readwrite.all",
            ),
            (
                ReadWrite {
                    offline: true,
                    shared: false,
                },
                "offline_access files.readwrite",
            ),
            (
                ReadWrite {
                    offline: true,
                    shared: true,
                },
                "offline_access files.readwrite.all",
            ),
        ];

        for (scope, s) in &scopes {
            assert_eq!(scope.to_scope_string(), *s);
        }
    }
}
