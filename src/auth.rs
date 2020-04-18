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
    /// [refresh_token]: ./struct.Token.html#structfield.refresh_token
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

/// Authentication to Microsoft Graph API
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth/auth-concepts?view=graph-rest-1.0)
// FIXME: Authorization/Auth
#[derive(Debug)]
pub struct Authentication {
    client: Client,
    client_id: String,
    permission: Permission,
    redirect_uri: String,
}

impl Authentication {
    /// Create an new instance for authentication with specified client identifier and permission.
    pub fn new(client_id: String, permission: Permission, redirect_uri: String) -> Self {
        Self::new_with_client(Client::new(), client_id, permission, redirect_uri)
    }

    /// Same as `Authentication::new` but with custom `Client`.
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

    /// Get the URL for web browser for token flow authentication.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-service?view=graph-rest-1.0)
    pub fn token_auth_url(&self) -> String {
        self.auth_url("token")
    }

    /// Get the URL for web browser for code flow authentication.
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
    ) -> Result<Token> {
        #[derive(Deserialize)]
        struct Resp {
            // FIXME
            // token_type: String,
            // expires_in: u64,
            // scope: String,
            access_token: String,
            refresh_token: Option<String>,
        }

        let resp = self
            .client
            .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            .form(params)
            .send()
            .await?;

        // Handle special error response.
        let resp: Resp = handle_oauth2_error_response(resp).await?.json().await?;

        if !require_refresh || resp.refresh_token.is_some() {
            Ok(Token {
                token: resp.access_token,
                refresh_token: resp.refresh_token,
                _private: (),
            })
        } else {
            Err(Error::unexpected_response("Missing field `refresh_token`"))
        }
    }

    /// Login using a code in code flow authentication.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#3-get-a-token)
    pub async fn login_with_code(&self, code: &str, client_secret: Option<&str>) -> Result<Token> {
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
    /// Panic if the current [`Authentication`][auth] is created with no
    /// [`offline_access`][offline_access] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [auth]: ./struct.Authentication.html
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    /// [refresh_token]: ./struct.Token.html#structfield.refresh_token
    pub async fn login_with_refresh_token(
        &self,
        refresh_token: &str,
        client_secret: Option<&str>,
    ) -> Result<Token> {
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

/// Access tokens
#[derive(Debug)]
pub struct Token {
    /// The access token used for authorization in requests.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-overview#what-is-an-access-token-and-how-do-i-use-it)
    pub token: String,
    /// The refresh token for refreshing (re-get) an access token when the previous one expired.
    ///
    /// This is only returned in code auth flow with [`offline_access`][offline_access] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    pub refresh_token: Option<String>,
    _private: (),
}
