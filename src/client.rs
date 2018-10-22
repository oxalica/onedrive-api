use super::error::*;
use super::resource::*;
use reqwest::{Client as RequestClient, RequestBuilder, Response, StatusCode};
use url::{PathSegmentsMut, Url};

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

        match self {
            &Read {
                shared: false,
                offline: false,
            } => "files.read",
            &Read {
                shared: false,
                offline: true,
            } => "files.read offline_access",
            &Read {
                shared: true,
                offline: false,
            } => "files.read.all",
            &Read {
                shared: true,
                offline: true,
            } => "files.read.all offline_access",
            &ReadWrite {
                shared: false,
                offline: false,
            } => "files.readwrite",
            &ReadWrite {
                shared: false,
                offline: true,
            } => "files.readwrite offline_access",
            &ReadWrite {
                shared: true,
                offline: false,
            } => "files.readwrite.all",
            &ReadWrite {
                shared: true,
                offline: true,
            } => "files.readwrite.all offline_access",
        }
    }
}

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

pub struct LoginClient {
    client: RequestClient,
    client_id: String,
    scope: Scope,
    redirect_uri: String,
}

impl LoginClient {
    pub fn new(client_id: String, scope: Scope, redirect_uri: String) -> Self {
        LoginClient {
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
        ).unwrap()
        .into_string()
    }

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#token-flow
    pub fn get_token_auth_url(&self) -> String {
        self.get_auth_url("token")
    }

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-1-get-an-authorization-code
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

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-2-redeem-the-code-for-access-tokens
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

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-3-get-a-new-access-token-or-refresh-token
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

    pub fn get_drive<'a>(&self, drive: impl Into<DriveLocation<'a>>) -> Result<Drive> {
        self.client
            .get(api_url![@drive &drive.into()])
            .bearer_auth(&self.token)
            .send()?
            .parse()
    }

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_list_children?view=odsp-graph-online
    pub fn list_children<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        match_tag: Option<&Tag>,
    ) -> Result<Option<Vec<DriveItem>>> {
        #[derive(Deserialize)]
        struct Response {
            value: Vec<DriveItem>,
            #[serde(rename = "@odata.nextLink")]
            next_link: Option<String>,
        }

        let fetch = |url: &str, match_tag: Option<&Tag>| -> Result<Option<Response>> {
            self.client
                .get(url)
                .bearer_auth(&self.token)
                .opt_header("if-none-match", match_tag)
                .send()?
                .parse_or_none()
        };

        let url = api_url![
            @drive &drive.into(),
            @item &item.into(),
            "children",
        ];
        match fetch(url.as_ref(), match_tag)? {
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

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_get?view=odsp-graph-online
    pub fn get_item<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        match_tag: Option<&Tag>,
    ) -> Result<Option<DriveItem>> {
        self.client
            .get(api_url![
                @drive &drive.into(),
                @item &item.into(),
            ]).bearer_auth(&self.token)
            .opt_header("if-none-match", match_tag)
            .send()?
            .parse_or_none()
    }

    const SMALL_FILE_SIZE: usize = 4 << 20; // 4 MB

    /// See also: https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content?view=odsp-graph-online
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
            ]).bearer_auth(&self.token)
            .body(data.to_owned())
            .send()?
            .parse()
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

use serde::de::DeserializeOwned;

trait ResponseExt: Sized {
    fn check_status(self) -> Result<Self>;
    fn parse<T: DeserializeOwned>(self) -> Result<T>;
    /// Allow `304 Not Modified`.
    fn parse_or_none<T: DeserializeOwned>(self) -> Result<Option<T>>;
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

    fn parse<T: DeserializeOwned>(self) -> Result<T> {
        Ok(self.check_status()?.json()?)
    }

    fn parse_or_none<T: DeserializeOwned>(self) -> Result<Option<T>> {
        if self.status() == StatusCode::NOT_MODIFIED {
            Ok(None)
        } else {
            self.parse().map(Option::Some)
        }
    }
}
