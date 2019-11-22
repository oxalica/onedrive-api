use crate::{
    api::{Api, ApiExt as _, SimpleApi},
    error::Error,
    util::ResponseExt as _,
};
use http::Request;
use serde::Deserialize;
use serde_urlencoded;

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
    /// This permission is required to get a refresh_token for long time access.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/permissions-reference#delegated-permissions-21)
    pub fn offline_access(mut self, offline_access: bool) -> Self {
        self.offline_access = offline_access;
        self
    }

    fn to_scope_str(&self) -> &'static str {
        macro_rules! cond_concat {
            ($($s:literal,)*) => { concat!($($s),*) };
            ($($s:literal,)* ($cond:expr, $t:literal, $f:literal), $($tt:tt)*) => {
                if $cond { cond_concat!($($s,)* $t, $($tt)*) }
                else { cond_concat!($($s,)* $f, $($tt)*) }
            };
        }

        cond_concat![
            (self.offline_access, "offline_access ", ""), // Postfix space here.
            (self.write, "files.readwrite", "files.read"),
            (self.access_shared, ".all", ""),
        ]
    }
}

/// The client for requests relative to authentication.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-overview?view=graph-rest-1.0)
// TODO: Rename to `Authentication`
#[derive(Debug)]
pub struct AuthClient {
    client_id: String,
    permission: Permission,
    redirect_uri: String,
}

impl AuthClient {
    /// Create a client for authorization.
    pub fn new(client_id: String, permission: Permission, redirect_uri: String) -> Self {
        AuthClient {
            client_id,
            permission,
            redirect_uri,
        }
    }

    fn get_auth_url(&self, response_type: &str) -> String {
        ::url::Url::parse_with_params(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            &[
                ("client_id", &self.client_id as &str),
                ("scope", self.permission.to_scope_str()),
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
    // TODO: Rename to `token_auth_url`
    pub fn get_token_auth_url(&self) -> String {
        self.get_auth_url("token")
    }

    /// Get the URL for web browser for code flow authentication.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#authorization-request)
    // TODO: Rename to `code_auth_url`
    pub fn get_code_auth_url(&self) -> String {
        self.get_auth_url("code")
    }

    fn request_authorize(
        &self,
        require_refresh: bool,
        params: &[(&str, &str)],
    ) -> impl Api<Response = Token> {
        #[derive(Deserialize)]
        struct Resp {
            // token_type: String,
            // expires_in: u64,
            // scope: String,
            access_token: String,
            refresh_token: Option<String>,
        }

        SimpleApi::new((|| {
            Ok(
                Request::post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
                    .body(serde_urlencoded::to_string(params)?.into_bytes())?,
            )
        })())
        .and_then(move |resp| {
            let resp: Resp = resp.parse()?;
            if !require_refresh || resp.refresh_token.is_some() {
                Ok(Token {
                    token: resp.access_token,
                    refresh_token: resp.refresh_token,
                    _private: (),
                })
            } else {
                Err(Error::unexpected_response("Missing field `refresh_token`"))
            }
        })
    }

    /// Login using a code in code flow authentication.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#3-get-a-token)
    pub fn login_with_code(
        &self,
        code: &str,
        client_secret: Option<&str>,
    ) -> impl Api<Response = Token> {
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
    }

    /// Login using a refresh token.
    ///
    /// This requires offline access, and will always returns new refresh token if success.
    ///
    /// # Panic
    /// Panic if the current [`AuthClient`][auth_client] is created with no
    /// [`offline_access`][offline_access] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [auth_client]: #
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    pub fn login_with_refresh_token(
        &self,
        refresh_token: &str,
        client_secret: Option<&str>,
    ) -> impl Api<Response = Token> {
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
    }
}

/// Access tokens from AuthClient.
#[derive(Debug)]
pub struct Token {
    /// The access token used for authorization in requests.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-overview#what-is-an-access-token-and-how-do-i-use-it)
    pub token: String,
    /// The refresh token for refreshing (re-get) an access token when the previous one expired.
    ///
    /// This is only provided in code authorization flow with
    /// [`offline_access`][offline_acccess] permission.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/auth-v2-user?view=graph-rest-1.0#5-use-the-refresh-token-to-get-a-new-access-token)
    ///
    /// [offline_access]: ./struct.Permission.html#method.offline_access
    pub refresh_token: Option<String>,
    _private: (),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_string() {
        for &write in &[false, true] {
            for &shared in &[false, true] {
                for &offline in &[false, true] {
                    assert_eq!(
                        Permission::new_read()
                            .write(write)
                            .access_shared(shared)
                            .offline_access(offline)
                            .to_scope_str(),
                        format!(
                            "{}{}{}",
                            if offline { "offline_access " } else { "" }, // Postfix space here.
                            if write {
                                "files.readwrite"
                            } else {
                                "files.read"
                            },
                            if shared { ".all" } else { "" },
                        ),
                        "When testing write={}, shared={}, offline={}",
                        write,
                        shared,
                        offline,
                    );
                }
            }
        }
    }
}
