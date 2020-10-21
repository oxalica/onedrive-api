use crate::{
    error::{Error, Result},
    util::handle_oauth2_error_response,
};
use reqwest::Client;
use serde::Deserialize;
use url::Url;

/// A list of the Microsoft Graph permissions that you want the user to consent to.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/permissions-reference#files-permissions)
#[derive(Clone, Debug, Default)]
pub struct Permission {
    write: bool,
    access_shared: bool,
    offline_access: bool,
}

impl Permission {
    /// Create a read-only permission.
    ///
    /// Note that the permission is at least to allow reading.
    pub fn new_read() -> Self {
        Self::default()
    }

    /// Set the write permission.
    pub fn write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    /// Set the permission to the shared files.
    pub fn access_shared(mut self, access_shared: bool) -> Self {
        self.access_shared = access_shared;
        self
    }

    /// Set whether allows offline access.
    ///
    /// This permission is required to get a [refresh_token][refresh_token] for long time access.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/permissions-reference#delegated-permissions-21)
    ///
    /// [refresh_token]: ./struct.TokenResponse.html#structfield.refresh_token
    pub fn offline_access(mut self, offline_access: bool) -> Self {
        self.offline_access = offline_access;
        self
    }

    #[rustfmt::skip]
    fn to_scope_string(&self) -> String {
        format!(
            "{}{}{}",
            if self.write { "files.readwrite" } else { "files.read" },
            if self.access_shared { ".all" } else { "" },
            if self.offline_access { " offline_access" } else { "" },
        )
    }
}

/// OAuth2 authentication and authorization basics for Microsoft Graph.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth/auth-concepts?view=graph-rest-1.0)
#[derive(Debug)]
pub struct Auth {
    client: Client,
    client_id: String,
    permission: Permission,
    redirect_uri: String,
}

impl Auth {
    /// Create an new instance for OAuth2 to Microsoft Graph
    /// with specified client identifier and permission.
    pub fn new(client_id: String, permission: Permission, redirect_uri: String) -> Self {
        Self::new_with_client(Client::new(), client_id, permission, redirect_uri)
    }

    /// Same as [`Auth::new`][auth_new] but with custom `reqwest::Client`.
    ///
    /// [auth_new]: #method.new
    pub fn new_with_client(
        client: Client,
        client_id: String,
        permission: Permission,
        redirect_uri: String,
    ) -> Self {
        Self {
            client,
            client_id,
            permission,
            redirect_uri,
        }
    }

    /// Get the `client_id` used to create this instance.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Get the `permission` used to create this instance.
    pub fn permission(&self) -> &Permission {
        &self.permission
    }

    /// Get the `redirect_uri` used to create this instance.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    fn auth_url(&self, response_type: &str) -> String {
        Url::parse_with_params(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            &[
                ("client_id", &*self.client_id),
                ("scope", &self.permission.to_scope_string()),
                ("redirect_uri", &self.redirect_uri),
                ("response_type", response_type),
            ],
        )
        .unwrap()
        .into_string()
    }

    /// Get the URL for web browser for code flow.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#authorization-request)
    pub fn code_auth_url(&self) -> String {
        self.auth_url("code")
    }

    async fn request_authorize(
        &self,
        require_refresh: bool,
        params: &[(&str, &str)],
    ) -> Result<TokenResponse> {
        let resp = self
            .client
            .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            .form(params)
            .send()
            .await?;

        // Handle special error response.
        let token_resp: TokenResponse = handle_oauth2_error_response(resp).await?.json().await?;

        if require_refresh && token_resp.refresh_token.is_none() {
            return Err(Error::unexpected_response("Missing field `refresh_token`"));
        }

        Ok(token_resp)
    }

    /// Login using a code.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#3-get-a-token)
    pub async fn login_with_code(
        &self,
        code: &str,
        client_secret: Option<&str>,
    ) -> Result<TokenResponse> {
        self.request_authorize(
            self.permission.offline_access,
            &[
                ("client_id", &self.client_id as &str),
                ("client_secret", client_secret.unwrap_or("")),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &self.redirect_uri),
            ],
        )
        .await
    }

    /// Login using a refresh token.
    ///
    /// This requires [`offline_access`][offline_access], and will **ALWAYS** return
    /// a new [`refresh_token`][refresh_token] if success.
    ///
    /// # Panic
    /// Panic if the current [`Auth`][auth] is created with no
    /// [`offline_access`][offline_access] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [auth]: ./struct.Auth.html
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    /// [refresh_token]: ./struct.TokenResponse.html#structfield.refresh_token
    pub async fn login_with_refresh_token(
        &self,
        refresh_token: &str,
        client_secret: Option<&str>,
    ) -> Result<TokenResponse> {
        assert!(
            self.permission.offline_access,
            "Refresh token requires offline_access permission."
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
        .await
    }
}

/// Tokens and some additional data returned by a successful authorization.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#token-response)
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct TokenResponse {
    /// Indicates the token type value. The only type that Azure AD supports is Bearer.
    pub token_type: String,
    /// A list of the Microsoft Graph permissions that the access_token is valid for.
    #[serde(deserialize_with = "space_separated_strings")]
    pub scope: Vec<String>,
    /// How long the access token is valid (in seconds).
    #[serde(rename = "expires_in")]
    pub expires_in_secs: u64,
    /// The access token used for authorization in requests.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-overview#what-is-an-access-token-and-how-do-i-use-it)
    pub access_token: String,
    /// The refresh token for refreshing (re-get) an access token when the previous one expired.
    ///
    /// This is only returned in code auth flow with [`offline_access`][offline_access] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    pub refresh_token: Option<String>,
}

fn space_separated_strings<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("space-separated strings")
        }

        fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(s.split(' ').map(|s| s.to_owned()).collect())
        }
    }

    deserializer.deserialize_str(Visitor)
}
