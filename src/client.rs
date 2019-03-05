use crate::query_option::{CollectionOption, ObjectOption};
use crate::resource::*;
use crate::{Error, Result};
use reqwest::{header, Client as RequestClient, RequestBuilder, Response};
use serde::{de, Deserialize, Serialize};
use std::ops::Range;
use url::{PathSegmentsMut, Url};

/// A list of the Microsoft Graph permissions that you want the user to consent to.
///
/// # See also
/// https://docs.microsoft.com/en-us/graph/permissions-reference#files-permissions
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
/// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0
#[derive(Clone, Debug)]
pub struct DriveLocation {
    inner: DriveLocationEnum,
}

#[derive(Clone, Debug)]
enum DriveLocationEnum {
    Me,
    User(String),
    Group(String),
    Site(String),
    Id(DriveId),
}

impl DriveLocation {
    /// Get current user's OneDrive
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-current-users-onedrive
    pub fn me() -> Self {
        Self {
            inner: DriveLocationEnum::Me,
        }
    }

    /// Get a user's OneDrive
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-a-users-onedrive
    pub fn from_user(id_or_principal_name: String) -> Self {
        Self {
            inner: DriveLocationEnum::User(id_or_principal_name),
        }
    }

    /// Get the document library associated with a group
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-the-document-library-associated-with-a-group
    pub fn from_group(group_id: String) -> Self {
        Self {
            inner: DriveLocationEnum::Group(group_id),
        }
    }

    /// Get the document library for a site
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-the-document-library-for-a-site
    pub fn from_site(site_id: String) -> Self {
        Self {
            inner: DriveLocationEnum::Site(site_id),
        }
    }

    /// Get a drive by ID
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0#get-a-drive-by-id
    pub fn from_id(drive_id: DriveId) -> Self {
        Self {
            inner: DriveLocationEnum::Id(drive_id),
        }
    }
}

impl From<DriveId> for DriveLocation {
    fn from(id: DriveId) -> Self {
        Self::from_id(id)
    }
}

/// Reference to a `DriveItem` in a drive.
/// It does not contains the drive information.
///
/// # See also
/// https://docs.microsoft.com/en-us/graph/api/driveitem-get?view=graph-rest-1.0
#[derive(Clone, Copy, Debug)]
pub struct ItemLocation<'a> {
    inner: ItemLocationEnum<'a>,
}

#[derive(Clone, Copy, Debug)]
enum ItemLocationEnum<'a> {
    Path(&'a str),
    Id(&'a str),
}

impl<'a> ItemLocation<'a> {
    /// A UNIX-like `/`-started absolute path to a file or directory in the drive,
    /// and the trailing `/` is optional.
    ///
    /// # Note
    /// If `path` contains invalid characters, it returns None.
    ///
    /// Special name on Windows like `CON` or `NUL` is tested to be permitted in API,
    /// but may still cause errors on Windows or OneDrive Online.
    /// These names will pass the check, but STRONGLY NOT recommended.
    ///
    /// # See also
    /// https://support.office.com/en-us/article/Invalid-file-names-and-file-types-in-OneDrive-OneDrive-for-Business-and-SharePoint-64883a5d-228e-48f5-b3d2-eb39e07630fa#invalidcharacters
    pub fn from_path(path: &'a str) -> Option<Self> {
        if path == "/" {
            Some(Self::root())
        } else if path.starts_with('/')
            && path[1..]
                .split_terminator('/')
                .all(|comp| !comp.is_empty() && FileName::new(comp).is_some())
        {
            Some(Self {
                inner: ItemLocationEnum::Path(path),
            })
        } else {
            None
        }
    }

    /// Item id from other API.
    pub fn from_id(item_id: &'a ItemId) -> Self {
        Self {
            inner: ItemLocationEnum::Id(item_id.as_ref()),
        }
    }

    /// The root directory item.
    pub fn root() -> Self {
        Self {
            inner: ItemLocationEnum::Path("/"),
        }
    }
}

impl<'a> From<&'a ItemId> for ItemLocation<'a> {
    fn from(id: &'a ItemId) -> Self {
        Self::from_id(id)
    }
}

#[derive(Debug)]
pub struct FileName(str);

impl FileName {
    /// Check and wrap the name for a file or a directory in OneDrive.
    ///
    /// Returns None if contains invalid characters.
    ///
    /// # See also
    /// [ItemLocation::from_path](ItemLocation::from_path)
    pub fn new(name: &str) -> Option<&Self> {
        const INVALID_CHARS: &str = r#""*:<>?/\|"#;

        if !name.is_empty() && !name.contains(|c| INVALID_CHARS.contains(c)) {
            Some(unsafe { &*(name as *const str as *const Self) })
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        unsafe { &*(self as *const Self as *const str) }
    }
}

trait ApiPathComponent {
    fn extend_into(&self, buf: &mut PathSegmentsMut);
}

impl ApiPathComponent for DriveLocation {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        use self::DriveLocationEnum::*;
        match &self.inner {
            Me => buf.push("drive"),
            User(id) => buf.extend(&["users", id, "drive"]),
            Group(id) => buf.extend(&["groups", id, "drive"]),
            Site(id) => buf.extend(&["sites", id, "drive"]),
            Id(id) => buf.extend(&["drives", id.as_ref()]),
        };
    }
}

impl ApiPathComponent for ItemLocation<'_> {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        use self::ItemLocationEnum::*;
        match &self.inner {
            Path("/") => buf.push("root"),
            Path(path) => buf.push(&["root:", path, ":"].join("")),
            Id(id) => buf.extend(&["items", id]),
        };
    }
}

impl ApiPathComponent for str {
    fn extend_into(&self, buf: &mut PathSegmentsMut) {
        buf.push(self);
    }
}

/// The client for requests relative to authentication.
///
/// # See also
/// https://docs.microsoft.com/en-us/graph/auth-overview?view=graph-rest-1.0
#[derive(Debug)]
pub struct AuthClient {
    client: RequestClient,
    client_id: String,
    scope: Scope,
    redirect_uri: String,
}

impl AuthClient {
    /// Create a client for authorization.
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
    /// TODO: Documentation
    pub fn get_token_auth_url(&self) -> String {
        self.get_auth_url("token")
    }

    /// Get the URL for web browser for code flow authentication.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#authorization-request
    pub fn get_code_auth_url(&self) -> String {
        self.get_auth_url("code")
    }

    fn request_authorize(&self, require_refresh: bool, params: &[(&str, &str)]) -> Result<Token> {
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
            return Err(Error::unexpected_response("Missing field `refresh_token`"));
        }

        Ok(Token {
            token: resp.access_token,
            refresh_token: resp.refresh_token,
            _private: (),
        })
    }

    /// Login using a code in code flow authentication.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#3-get-a-token
    pub fn login_with_code(&self, code: &str, client_secret: Option<&str>) -> Result<Token> {
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
    ///
    /// This requires offline access, and will always returns new refresh token if success.
    ///
    /// # Panic
    /// Panic if the `scope` given in `Client::new` has no `offline_access` scope.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token
    pub fn login_with_refresh_token(
        &self,
        refresh_token: &str,
        client_secret: Option<&str>,
    ) -> Result<Token> {
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

/// Access tokens from AuthClient.
#[derive(Debug)]
pub struct Token {
    pub token: String,
    pub refresh_token: Option<String>,
    _private: (),
}

macro_rules! api_url {
    (@$init:expr; $($seg:expr),* $(,)*) => {
        {
            let mut url = Url::parse($init).unwrap();
            {
                let mut buf = url.path_segments_mut().unwrap();
                $(ApiPathComponent::extend_into($seg, &mut buf);)*
            } // End borrowing of `url`
            url
        }
    };
    ($($t:tt)*) => {
        api_url!(@"https://graph.microsoft.com/v1.0"; $($t)*)
    };
}

macro_rules! api_path {
    ($($t:tt)*) => {
        api_url![@"path://"; $($t)*].path()
    };
}

/// The authorized client to access OneDrive resources in a specified Drive.
#[derive(Debug)]
pub struct DriveClient {
    client: RequestClient,
    token: String,
    drive: DriveLocation,
}

impl DriveClient {
    /// Create a DriveClient to perform operations in a Drive.
    pub fn new(token: String, drive: impl Into<DriveLocation>) -> Self {
        DriveClient {
            client: RequestClient::new(),
            token,
            drive: drive.into(),
        }
    }

    /// Get `Drive`
    ///
    /// Retrieve the properties and relationships of a `Drive` resource.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0
    pub fn get_drive_with_option(&self, option: ObjectOption<Drive>) -> Result<Drive> {
        self.client
            .get(api_url![&self.drive])
            .query(&option.params().collect::<Vec<_>>())
            .bearer_auth(&self.token)
            .send()?
            .parse()
    }

    /// Shortcut to `get_drive_with_option` with default parameters.
    pub fn get_drive(&self) -> Result<Drive> {
        self.get_drive_with_option(Default::default())
    }

    /// List children of a `DriveItem`
    ///
    /// Return a collection of `DriveItem`s in the children relationship of a `DriveItem`.
    ///
    /// # Note
    /// Will return `Ok(None)` if `if_none_match` is set and matches the item.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-list-children?view=graph-rest-1.0
    pub fn list_children_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        if_none_match: Option<&Tag>,
        option: CollectionOption<DriveItem>,
    ) -> Result<Option<ListChildrenFetcher>> {
        self.client
            .get(api_url![&self.drive, &item.into(), "children"])
            .query(&option.params().collect::<Vec<_>>())
            .bearer_auth(&self.token)
            .opt_header(header::IF_NONE_MATCH, if_none_match)
            .send()?
            .parse_optional()
            .map(|opt_resp| opt_resp.map(|resp| ListChildrenFetcher::new(self, resp)))
    }

    /// Shortcut to `list_children_with_option` with default params and fetch all.
    pub fn list_children<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        if_none_match: Option<&Tag>,
    ) -> Result<Option<Vec<DriveItem>>> {
        self.list_children_with_option(item.into(), if_none_match, Default::default())?
            .map(|fetcher| fetcher.fetch_all())
            .transpose()
    }

    /// Get a DriveItem resource
    ///
    /// Retrieve the metadata for a `DriveItem` in a `Drive` by file system path or ID.
    ///
    /// # Errors
    /// Will return `Ok(None)` if `if_none_match` is set and matches the item .
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-get?view=graph-rest-1.0
    pub fn get_item_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        if_none_match: Option<&Tag>,
        option: ObjectOption<DriveItem>,
    ) -> Result<Option<DriveItem>> {
        self.client
            .get(api_url![&self.drive, &item.into()])
            .query(&option.params().collect::<Vec<_>>())
            .bearer_auth(&self.token)
            .opt_header(header::IF_NONE_MATCH, if_none_match)
            .send()?
            .parse_optional()
    }

    /// Shortcut to `get_item_with_option` with default parameters.
    pub fn get_item<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        if_none_match: Option<&Tag>,
    ) -> Result<Option<DriveItem>> {
        self.get_item_with_option(item.into(), if_none_match, Default::default())
    }

    /// Create a new folder in a drive
    ///
    /// Create a new folder or `DriveItem` in a `Drive` with a specified parent item or path.
    ///
    /// # Errors
    /// Will return `Err` with HTTP CONFLICT if the target already exists.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-post-children?view=graph-rest-1.0
    pub fn create_folder<'a>(
        &self,
        parent_item: impl Into<ItemLocation<'a>>,
        name: &FileName,
    ) -> Result<DriveItem> {
        #[derive(Serialize)]
        struct Folder {}

        #[derive(Serialize)]
        struct Request<'a> {
            name: &'a str,
            folder: Folder,
            /// https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0#instance-attributes
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: &'a str,
        }

        self.client
            .post(api_url![&self.drive, &parent_item.into(), "children"])
            .bearer_auth(&self.token)
            .json(&Request {
                name: name.as_str(),
                folder: Folder {},
                conflict_behavior: "fail", // TODO
            })
            .send()?
            .parse()
    }

    const UPLOAD_SMALL_LIMIT: usize = 4_000_000; // 4 MB

    /// Upload or replace the contents of a `DriveItem`
    ///
    /// The simple upload API allows you to provide the contents of a new file or
    /// update the contents of an existing file in a single API call. This method
    /// only supports files up to 4MB in size.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-put-content?view=graph-rest-1.0
    pub fn upload_small<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        data: &[u8],
    ) -> Result<DriveItem> {
        assert!(
            data.len() <= Self::UPLOAD_SMALL_LIMIT,
            "Data too large for upload_small ({} B > {} B)",
            data.len(),
            Self::UPLOAD_SMALL_LIMIT,
        );

        self.client
            .put(api_url![&self.drive, &item.into(), "content"])
            .bearer_auth(&self.token)
            .body(data.to_owned())
            .send()?
            .parse()
    }

    /// Create an upload session
    ///
    /// Create an upload session to allow your app to upload files up to
    /// the maximum file size. An upload session allows your app to
    /// upload ranges of the file in sequential API requests, which allows
    /// the transfer to be resumed if a connection is dropped
    /// while the upload is in progress.
    ///
    /// # Errors
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set
    /// but does not match the item.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#create-an-upload-session
    pub fn new_upload_session<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        overwrite: bool,
        if_match: Option<&Tag>,
    ) -> Result<UploadSession> {
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
            .post(api_url![&self.drive, &item.into(), "createUploadSession"])
            .opt_header(header::IF_MATCH, if_match)
            .bearer_auth(&self.token)
            .json(&Request {
                item: Item {
                    conflict_behavior: if overwrite { "overwrite" } else { "fail" },
                },
            })
            .send()?
            .parse()
    }

    /// Resuming an in-progress upload
    ///
    /// Query the status of the upload to find out which byte ranges
    /// have been received previously.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#resuming-an-in-progress-upload
    pub fn get_upload_session(&self, upload_url: &str) -> Result<UploadSession> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct UploadSessionResponse {
            // TODO: Incompleted
            upload_url: Option<String>,
            next_expected_ranges: Vec<ExpectRange>,
            // expiration_date_time: Timestamp,
        }

        self.client
            .get(upload_url)
            .send()?
            .parse::<UploadSessionResponse>()
            .map(|resp| UploadSession {
                upload_url: upload_url.to_owned(),
                next_expected_ranges: resp.next_expected_ranges,
            })
    }

    /// Cancel the upload session
    ///
    /// This cleans up the temporary file holding the data previously uploaded.
    /// This should be used in scenarios where the upload is aborted, for example,
    /// if the user cancels the transfer.
    ///
    /// Temporary files and their accompanying upload session are automatically
    /// cleaned up after the expirationDateTime has passed. Temporary files may
    /// not be deleted immedately after the expiration time has elapsed.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#cancel-the-upload-session
    pub fn delete_upload_session(&self, sess: &UploadSession) -> Result<()> {
        self.client
            .delete(&sess.upload_url)
            .send()?
            .parse_no_content()
    }

    const UPLOAD_SESSION_PART_LIMIT: usize = 60 << 20; // 60 MiB

    /// Upload bytes to the upload session
    ///
    /// You can upload the entire file, or split the file into multiple byte ranges,
    /// as long as the maximum bytes in any given request is less than 60 MiB.
    /// The fragments of the file must be uploaded sequentially in order. Uploading
    /// fragments out of order will result in an error.
    ///
    /// Note: If your app splits a file into multiple byte ranges, the size of each
    /// byte range MUST be a multiple of 320 KiB (327,680 bytes). Using a fragment
    /// size that does not divide evenly by 320 KiB will result in errors committing
    /// some files.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#upload-bytes-to-the-upload-session
    pub fn upload_to_session(
        &self,
        session: &UploadSession,
        data: &[u8],
        remote_range: Range<usize>,
        total_size: usize,
    ) -> Result<Option<DriveItem>> {
        // FIXME: https://github.com/rust-lang/rust-clippy/issues/3807
        #[allow(clippy::len_zero)]
        {
            assert!(
                remote_range.len() > 0 && remote_range.end <= total_size,
                "Invalid range",
            );
        }
        assert_eq!(
            data.len(),
            remote_range.end - remote_range.start,
            "Length mismatch"
        );
        assert!(
            data.len() <= Self::UPLOAD_SESSION_PART_LIMIT,
            "Data too large for one part ({} B > {} B)",
            data.len(),
            Self::UPLOAD_SESSION_PART_LIMIT,
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
            .parse_optional()
    }

    /// Copy a DriveItem.
    ///
    /// Asynchronously creates a copy of an driveItem (including any children),
    /// under a new parent item or with a new name.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-copy?view=graph-rest-1.0
    pub fn copy<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: &FileName,
    ) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            parent_reference: ItemReference<'a>,
            name: &'a str,
        }

        self.client
            .post(api_url![&self.drive, &source_item.into(), "copy"])
            .bearer_auth(&self.token)
            .json(&Request {
                parent_reference: ItemReference {
                    path: api_path![&self.drive, &dest_folder.into()],
                },
                name: dest_name.as_str(),
            })
            .send()?
            .parse_no_content() // TODO: Handle async copy
    }

    /// Move a DriveItem to a new folder
    ///
    /// This is a special case of the Update method. Your app can combine
    /// moving an item to a new container and updating other properties of
    /// the item into a single request.
    ///
    /// Note: Items cannot be moved between Drives using this request.
    ///
    /// # Errors
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set
    /// but doesn't match the item.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-move?view=graph-rest-1.0
    pub fn move_<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_directory: impl Into<ItemLocation<'b>>,
        dest_name: Option<&FileName>,
        if_match: Option<&Tag>,
    ) -> Result<DriveItem> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            parent_reference: ItemReference<'a>,
            name: Option<&'a str>,
        }

        self.client
            .patch(api_url![&self.drive, &source_item.into()])
            .bearer_auth(&self.token)
            .opt_header(header::IF_MATCH, if_match)
            .json(&Request {
                parent_reference: ItemReference {
                    path: api_path![&self.drive, &dest_directory.into()],
                },
                name: dest_name.map(FileName::as_str),
            })
            .send()?
            .parse()
    }

    /// Delete a DriveItem
    ///
    /// Delete a `DriveItem` by using its ID or path. Note that deleting items using
    /// this method will move the items to the recycle bin instead of permanently
    /// deleting the item.
    ///
    /// # Errors
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set but
    /// does not match the item.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-delete?view=graph-rest-1.0
    pub fn delete<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        if_match: Option<&Tag>,
    ) -> Result<()> {
        self.client
            .delete(api_url![&self.drive, &item.into()])
            .bearer_auth(&self.token)
            .opt_header(header::IF_MATCH, if_match)
            .send()?
            .parse_no_content()
    }

    /// Track changes for a folder from initial state (empty state) to snapshot of current states.
    ///
    /// This method allows your app to track changes to a drive and its children over time.
    /// Deleted items are returned with the deleted facet. Items with this property set
    /// should be removed from your local state.
    ///
    /// Note: you should only delete a folder locally if it is empty after
    /// syncing all the changes.
    ///
    /// # Return
    /// The fetcher for fetching all changes from initial state (empty) to the snapshot of
    /// current states.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0
    pub fn track_changes_from_initial_with_option<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
        option: CollectionOption<DriveItem>,
    ) -> Result<TrackChangeFetcher> {
        self.client
            .get(&api_url![&self.drive, &folder.into(), "delta"].into_string())
            .query(&option.params().collect::<Vec<_>>())
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .map(|resp| TrackChangeFetcher::new(self, resp))
    }

    /// Shortcut to `track_changes_from_initial_with_option` with default parameters.
    pub fn track_changes_from_initial<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
    ) -> Result<(Vec<DriveItem>, String)> {
        self.track_changes_from_initial_with_option(folder.into(), Default::default())?
            .fetch_all()
    }

    /// Track changes for a folder from snapshot (delta url) to snapshot of current states.
    ///
    /// # See also
    /// `track_changes_from_initial_with_option()`
    pub fn track_changes_from_delta_url(&self, delta_url: &str) -> Result<TrackChangeFetcher> {
        self.client
            .get(delta_url)
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .map(|resp| TrackChangeFetcher::new(self, resp))
    }

    /// Get a delta url representing the snapshot of current states.
    ///
    /// # See also
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0
    pub fn get_latest_delta_url<'a>(&self, folder: impl Into<ItemLocation<'a>>) -> Result<String> {
        self.client
            .get(&api_url![&self.drive, &folder.into(), "delta"].into_string())
            .query(&[("token", "latest")])
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .and_then(|resp: DriveItemCollectionResponse| {
                resp.delta_url.ok_or_else(|| {
                    Error::unexpected_response(
                        "Missing field `@odata.deltaLink` for getting latest delta",
                    )
                })
            })
    }
}

#[derive(Debug, Deserialize)]
struct DriveItemCollectionResponse {
    value: Option<Vec<DriveItem>>,
    #[serde(rename = "@odata.nextLink")]
    next_url: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta_url: Option<String>,
}

#[derive(Debug)]
struct DriveItemFetcher {
    client: reqwest::Client,
    token: String,
    response: DriveItemCollectionResponse,
}

impl DriveItemFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            client: client.client.clone(),
            token: client.token.clone(),
            response,
        }
    }

    fn get_next_url(&self) -> Option<&str> {
        match (&self.response.value, &self.response.next_url) {
            (None, Some(url)) => Some(url),
            _ => None,
        }
    }

    fn fetch_next(&mut self) -> Option<Result<Vec<DriveItem>>> {
        if let Some(v) = self.response.value.take() {
            return Some(Ok(v));
        }

        // Not `take` here. Will remain unchanged if failed to fetch.
        let url = self.response.next_url.as_ref()?;
        match (|| {
            self.client
                .get(url)
                .bearer_auth(&self.token)
                .send()?
                .parse::<DriveItemCollectionResponse>()
        })() {
            Err(err) => Some(Err(err)),
            Ok(DriveItemCollectionResponse {
                next_url: Some(_),
                value: None,
                ..
            }) => Some(Err(Error::unexpected_response(
                "Missing field `value` when not finished",
            ))),
            Ok(resp) => {
                self.response = resp;
                Some(Ok(self.response.value.take()?))
            }
        }
    }

    fn fetch_all(&mut self) -> Result<Vec<DriveItem>> {
        let mut buf = vec![];
        while let Some(ret) = self.fetch_next() {
            buf.append(&mut ret?);
        }
        Ok(buf)
    }
}

/// The page fetcher for `list_children` operation with `Iterator` interface.
#[derive(Debug)]
pub struct ListChildrenFetcher {
    fetcher: DriveItemFetcher,
}

impl ListChildrenFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(client, response),
        }
    }

    /// Resume a fetching process from url from `ListChildrenFetcher::get_next_url`;
    pub fn resume_from(client: &DriveClient, next_url: String) -> Self {
        Self::new(
            client,
            DriveItemCollectionResponse {
                value: None,
                next_url: Some(next_url),
                delta_url: None,
            },
        )
    }

    /// Try to get the url to the next page.
    ///
    /// Used for resuming the fetching progress.
    ///
    /// # Error
    /// Will success only if there are more pages and the first page is already readed.
    ///
    /// # Note
    /// The first page data from `DriveClient::list_children[_with_option]`
    /// will be cached and have no idempotent url to resume/re-fetch.
    pub fn get_next_url(&self) -> Option<&str> {
        self.fetcher.get_next_url()
    }

    /// Fetch all rest pages and return all items concated.
    ///
    /// # Errors
    /// Will return `Err` if any error occurs during fetching.
    ///
    /// Note that you will lose all progress unless all requests are success,
    /// so it is preferred to use `Iterator::next` to make it more error-tolerant.
    pub fn fetch_all(mut self) -> Result<Vec<DriveItem>> {
        self.fetcher.fetch_all()
    }
}

impl Iterator for ListChildrenFetcher {
    type Item = Result<Vec<DriveItem>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.fetcher.fetch_next()
    }
}

#[derive(Debug)]
pub struct TrackChangeFetcher {
    fetcher: DriveItemFetcher,
}

impl TrackChangeFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(client, response),
        }
    }

    /// Resume a fetching process from url from `TrackChangeFetcher::get_next_url`;
    pub fn resume_from(client: &DriveClient, next_url: String) -> Self {
        Self {
            fetcher: DriveItemFetcher {
                client: client.client.clone(),
                token: client.token.clone(),
                response: DriveItemCollectionResponse {
                    value: None,
                    delta_url: None,
                    next_url: Some(next_url),
                },
            },
        }
    }

    /// Try to get the url to the next page.
    ///
    /// Used for resuming the fetching progress.
    ///
    /// # Error
    /// Will success only if there are more pages and the first page is already readed.
    ///
    /// # Note
    /// The first page data from `DriveClient::track_changes_from_initial(_with_option)`
    /// will be cached and have no idempotent url to resume/re-fetch.
    pub fn get_next_url(&self) -> Option<&str> {
        self.fetcher.get_next_url()
    }

    /// Try to the delta url representing a snapshot of current track change operation.
    ///
    /// Used for tracking changes from this snapshot (rather than initial) later,
    /// using `DriveClient::track_changes_from_delta_url`.
    ///
    /// # Error
    /// Will success only if there are no more pages.
    ///
    /// # See also
    /// `DriveClient::track_changes_from_delta_url`
    ///
    /// https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#example-last-page-in-a-set
    pub fn get_delta_url(&self) -> Option<&str> {
        match &self.fetcher.response {
            DriveItemCollectionResponse {
                value: None,
                delta_url: Some(url),
                ..
            } => Some(url),
            _ => None,
        }
    }

    /// Fetch all rest pages and return all items concated with a delta url.
    ///
    /// # Errors
    /// Will return `Err` if any error occurs during fetching.
    ///
    /// Note that you will lose all progress unless all requests are success,
    /// so it is preferred to use `Iterator::next` to make it more error-tolerant.
    pub fn fetch_all(mut self) -> Result<(Vec<DriveItem>, String)> {
        let v = self.fetcher.fetch_all()?;
        // Must not be None if `fetch_all` succeed.
        Ok((v, self.fetcher.response.delta_url.unwrap()))
    }
}

impl Iterator for TrackChangeFetcher {
    type Item = Result<Vec<DriveItem>>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.fetcher.fetch_next(), &self.fetcher.response.delta_url) {
            (None, None) => Some(Err(Error::unexpected_response(
                "Missing field `@odata.deltaLink` for the last page",
            ))),
            (ret, _) => ret,
        }
    }
}

#[derive(Serialize)]
struct ItemReference<'a> {
    path: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadSession {
    // TODO: Incompleted
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
    fn parse_optional<T: de::DeserializeOwned>(self) -> Result<Option<T>>;
    fn parse_no_content(self) -> Result<()>;
}

impl ResponseExt for Response {
    fn check_status(mut self) -> Result<Self> {
        match self.error_for_status_ref() {
            Ok(_) => Ok(self),
            Err(source) => {
                #[derive(Deserialize)]
                struct ErrorResponse {
                    error: ErrorObject,
                }

                let response: ErrorResponse = self.json()?; // Throw network or serialization error first.
                Err(Error::from_response(source, Some(response.error)))
            }
        }
    }

    fn parse<T: de::DeserializeOwned>(self) -> Result<T> {
        Ok(self.check_status()?.json()?)
    }

    fn parse_optional<T: de::DeserializeOwned>(self) -> Result<Option<T>> {
        use reqwest::StatusCode;

        match self.status() {
            StatusCode::NOT_MODIFIED | StatusCode::ACCEPTED => Ok(None),
            _ => Ok(Some(self.parse()?)),
        }
    }

    fn parse_no_content(self) -> Result<()> {
        self.check_status()?;
        Ok(())
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

    #[test]
    fn test_api_url() {
        assert_eq!(
            api_url!["a", &DriveLocation::me(), "b"].path(),
            "/v1.0/a/drive/b",
        );

        let mock_drive_id = DriveId::new("1234".to_owned());
        assert_eq!(
            api_path![&DriveLocation::from_id(mock_drive_id)],
            "/drives/1234",
        );

        assert_eq!(
            api_path![&ItemLocation::from_path("/dir/file name").unwrap()],
            "/root:%2Fdir%2Ffile%20name:",
        );
    }

    #[test]
    fn test_path_name_check() {
        let invalid_names = ["", ".*?", "a|b", "a<b>b", ":run", "/", "\\"];
        let valid_names = [
            "QAQ",
            "0",
            ".",
            "a-a：", // Unicode colon "\u{ff1a}"
            "魔理沙",
        ];

        let check_name = |s: &str| FileName::new(s).is_some();
        let check_path = |s: &str| ItemLocation::from_path(s).is_some();

        for s in &valid_names {
            assert!(check_name(s), "{}", s);
            let path = format!("/{}", s);
            assert!(check_path(&path), "{}", path);

            for s2 in &valid_names {
                let mut path = format!("/{}/{}", s, s2);
                assert!(check_path(&path), "{}", path);
                path.push('/'); // Trailing
                assert!(check_path(&path), "{}", path);
            }
        }

        for s in &invalid_names {
            assert!(!check_name(s), "{}", s);

            // `/` and `/xx/` is valid and is tested below.
            if s.is_empty() {
                continue;
            }

            let path = format!("/{}", s);
            assert!(!check_path(&path), "{}", path);

            for s2 in &valid_names {
                let path = format!("/{}/{}", s2, s);
                assert!(!check_path(&path), "{}", path);
            }
        }

        assert!(check_path("/"));
        assert!(check_path("/a"));
        assert!(check_path("/a/"));
        assert!(check_path("/a/b"));
        assert!(check_path("/a/b/"));

        assert!(!check_path(""));
        assert!(!check_path("/a/b//"));
        assert!(!check_path("a"));
        assert!(!check_path("a/"));
        assert!(!check_path("//"));
    }
}
