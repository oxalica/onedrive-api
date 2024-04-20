use std::fmt;

use crate::{
    error::{Error, Result},
    util::handle_oauth2_error_response,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

/// A list of the Microsoft Graph permissions that you want the user to consent to.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/permissions-reference#files-permissions)
#[derive(Clone, Copy, Debug, Default)]
pub struct Permission {
    write: bool,
    access_shared: bool,
    offline_access: bool,
}

impl Permission {
    /// Create a read-only permission.
    ///
    /// Note that the permission is at least to allow reading.
    #[must_use]
    pub fn new_read() -> Self {
        Self::default()
    }

    /// Set the write permission.
    #[must_use]
    pub fn write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    /// Set the permission to the shared files.
    #[must_use]
    pub fn access_shared(mut self, access_shared: bool) -> Self {
        self.access_shared = access_shared;
        self
    }

    /// Set whether allows offline access.
    ///
    /// This permission is required to get a [`TokenResponse::refresh_token`] for long time access.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/permissions-reference#delegated-permissions-21)
    #[must_use]
    pub fn offline_access(mut self, offline_access: bool) -> Self {
        self.offline_access = offline_access;
        self
    }

    #[must_use]
    #[rustfmt::skip]
    fn to_scope_string(self) -> String {
        format!(
            "{}{}{}",
            if self.write { "files.readwrite" } else { "files.read" },
            if self.access_shared { ".all" } else { "" },
            if self.offline_access { " offline_access" } else { "" },
        )
    }
}

/// Control who can sign into the application.
///
/// It must match the target audience configuration of registered application.
///
/// See: <https://learn.microsoft.com/en-us/graph/auth-v2-user?tabs=http#parameters>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tenant {
    /// For both Microsoft accounts and work or school accounts.
    ///
    /// # Notes
    ///
    /// This is only allowed for application with type `AzureADandPersonalMicrosoftAccount`
    /// (Accounts in any organizational directory (Any Microsoft Entra directory - Multitenant) and
    /// personal Microsoft accounts (e.g. Skype, Xbox)). If the coresponding application by
    /// Client ID does not have this type, authentications will fail unconditionally.
    ///
    /// See:
    /// <https://learn.microsoft.com/en-us/entra/identity-platform/supported-accounts-validation>
    Common,
    /// For work or school accounts only.
    Organizations,
    /// For Microsoft accounts only.
    Consumers,
    /// Tenant identifiers such as the tenant ID or domain name.
    ///
    /// See: <https://learn.microsoft.com/en-us/entra/identity-platform/v2-protocols#endpoints>
    Issuer(String),
}

impl Tenant {
    fn to_issuer(&self) -> &str {
        match self {
            Tenant::Common => "common",
            Tenant::Organizations => "organizations",
            Tenant::Consumers => "consumers",
            Tenant::Issuer(s) => s,
        }
    }
}

/// OAuth2 authentication and authorization basics for Microsoft Graph.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth/auth-concepts?view=graph-rest-1.0)
#[derive(Debug, Clone)]
pub struct Auth {
    client: Client,
    client_id: String,
    permission: Permission,
    redirect_uri: String,
    tenant: Tenant,
}

impl Auth {
    /// Create an new instance for OAuth2 to Microsoft Graph
    /// with specified client identifier and permission.
    pub fn new(
        client_id: impl Into<String>,
        permission: Permission,
        redirect_uri: impl Into<String>,
        tenant: Tenant,
    ) -> Self {
        Self::new_with_client(Client::new(), client_id, permission, redirect_uri, tenant)
    }

    /// Same as [`Auth::new`][auth_new] but with custom `reqwest::Client`.
    ///
    /// [auth_new]: #method.new
    pub fn new_with_client(
        client: Client,
        client_id: impl Into<String>,
        permission: Permission,
        redirect_uri: impl Into<String>,
        tenant: Tenant,
    ) -> Self {
        Self {
            client,
            client_id: client_id.into(),
            permission,
            redirect_uri: redirect_uri.into(),
            tenant,
        }
    }

    /// Get the `client` used to create this instance.
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the `client_id` used to create this instance.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Get the `permission` used to create this instance.
    #[must_use]
    pub fn permission(&self) -> &Permission {
        &self.permission
    }

    /// Get the `redirect_uri` used to create this instance.
    #[must_use]
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Get the `tenant` used to create this instance.
    #[must_use]
    pub fn tenant(&self) -> &Tenant {
        &self.tenant
    }

    #[must_use]
    fn endpoint_url(&self, endpoint: &str) -> Url {
        let mut url = Url::parse("https://login.microsoftonline.com").unwrap();
        url.path_segments_mut().unwrap().extend([
            self.tenant.to_issuer(),
            "oauth2",
            "v2.0",
            endpoint,
        ]);
        url
    }

    /// Get the URL for web browser for code flow.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#authorization-request)
    #[must_use]
    pub fn code_auth_url(&self) -> Url {
        let mut url = self.endpoint_url("authorize");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("scope", &self.permission.to_scope_string())
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("response_type", "code");
        url
    }

    async fn request_token<'a>(
        &self,
        require_refresh: bool,
        params: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> Result<TokenResponse> {
        let url = self.endpoint_url("token");
        let params = params.collect::<Vec<_>>();
        let resp = self.client.post(url).form(&params).send().await?;

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
        client_credential: &ClientCredential,
    ) -> Result<TokenResponse> {
        self.request_token(
            self.permission.offline_access,
            [
                ("client_id", &self.client_id as &str),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &self.redirect_uri),
            ]
            .into_iter()
            .chain(client_credential.params()),
        )
        .await
    }

    /// Login using a refresh token.
    ///
    /// This requires [`offline_access`][offline_access], and will **ALWAYS** return
    /// a new [`refresh_token`][refresh_token] if success.
    ///
    /// # Panics
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
        client_credential: &ClientCredential,
    ) -> Result<TokenResponse> {
        assert!(
            self.permission.offline_access,
            "Refresh token requires offline_access permission."
        );

        self.request_token(
            true,
            [
                ("client_id", &self.client_id as &str),
                ("grant_type", "refresh_token"),
                ("redirect_uri", &self.redirect_uri),
                ("refresh_token", refresh_token),
            ]
            .into_iter()
            .chain(client_credential.params()),
        )
        .await
    }
}

/// Credential of client for code redeemption.
///
/// See:
/// <https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow#redeem-a-code-for-an-access-token>
#[derive(Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientCredential {
    /// Nothing.
    ///
    /// This is the usual case for non-confidential native apps.
    #[default]
    None,
    /// The application secret that you created in the app registration portal for your app.
    ///
    /// Don't use the application secret in a native app or single page app because a
    /// `client_secret` can't be reliably stored on devices or web pages.
    ///
    /// See:
    /// <https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow#request-an-access-token-with-a-client_secret>
    Secret(String),
    /// An assertion, which is a JSON web token (JWT), that you need to create and sign with the
    /// certificate you registered as credentials for your application.
    ///
    /// See:
    /// <https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow#request-an-access-token-with-a-certificate-credential>
    Assertion(String),
}

impl fmt::Debug for ClientCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Secret(_) => f.debug_struct("Secret").finish_non_exhaustive(),
            Self::Assertion(_) => f.debug_struct("Assertion").finish_non_exhaustive(),
        }
    }
}

impl ClientCredential {
    fn params(&self) -> impl Iterator<Item = (&str, &str)> {
        let (a, b) = match self {
            ClientCredential::None => (None, None),
            ClientCredential::Secret(s) => (Some(("client_secret", &**s)), None),
            ClientCredential::Assertion(s) => (
                Some((
                    "client_assertion_type",
                    "urn:ietf:params:oauth:client-assertion-type:jwt-bearer",
                )),
                Some(("client_assertion", &**s)),
            ),
        };
        a.into_iter().chain(b)
    }
}

/// Tokens and some additional data returned by a successful authorization.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#token-response)
#[derive(Clone, Deserialize, Serialize)]
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

impl fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenResponse")
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .field("expires_in_secs", &self.expires_in_secs)
            .finish_non_exhaustive()
    }
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
            Ok(s.split(' ').map(Into::into).collect())
        }
    }

    deserializer.deserialize_str(Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_url() {
        let perm = Permission::new_read().write(true).offline_access(true);
        let auth = Auth::new(
            "some-client-id",
            perm,
            "http://example.com",
            Tenant::Consumers,
        );
        assert_eq!(
            auth.code_auth_url().as_str(),
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id=some-client-id&scope=files.readwrite+offline_access&redirect_uri=http%3A%2F%2Fexample.com&response_type=code",
        );
    }
}
